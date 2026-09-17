//! The recording commands: inspect, fingerprint, detect, verify, report.
//!
//! Each command answers with text for a person or one JSON document for a pipeline, and a status: 0 when
//! the answer is a yes, 1 when it is a no, 2 when the question could not be asked. A "no" only exists for
//! `verify`, where it means a compared subject differs, and for `detect`, where it means a file could not
//! be read; neither is a judgement about a recording, both are facts about bytes.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use adiungere_isobmff::FileSource;
use adiungere_manifest::wording::{Phrase, render};
use adiungere_manifest::{
    ExportClass, Manifest, Masking, Operation, Outcome, Producer, Scope, SourceRecord, SourceTrack, Subject,
    TrackFingerprint, TrackKind, VendorBox, VendorLocation, Verification,
};
use adiungere_scan::{Recording, Scan, ScanCache, Signal, parse_name, scan};

/// A rendered answer and whether it is a no.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rendered {
    /// The text to print, ending with a newline.
    pub text: String,
    /// Whether the answer is a no.
    pub negative: bool,
}

/// Anything that stops a recording command before it can answer.
#[derive(Debug)]
pub enum MediaFailure {
    /// A file could not be opened.
    Open {
        /// The path.
        path: PathBuf,
        /// What the operating system said.
        cause: adiungere_isobmff::SourceError,
    },
    /// A file could not be read as a container, or a fingerprint could not be computed.
    Read {
        /// The path.
        path: PathBuf,
        /// What went wrong.
        cause: adiungere_manifest::Error,
    },
    /// A manifest could not be read.
    Manifest {
        /// The path.
        path: PathBuf,
        /// What went wrong.
        cause: String,
    },
    /// A file could not be written.
    Write {
        /// The path.
        path: PathBuf,
        /// What the operating system said.
        cause: std::io::Error,
    },
    /// The answer could not be serialised.
    Serialisation {
        /// What went wrong.
        cause: serde_json::Error,
    },
    /// A recording does not hold what the selection asks for.
    Selection {
        /// The path.
        path: PathBuf,
        /// What is missing.
        reason: String,
    },
    /// The export could not be written, or was stopped.
    Export {
        /// The path of the output.
        path: PathBuf,
        /// What went wrong.
        cause: adiungere_isobmff::Error,
    },
    /// A signing, a time stamp or a reading of credentials failed.
    Provenance {
        /// What went wrong.
        cause: String,
    },
    /// A path the export would write is a recording it reads.
    Overwrite {
        /// The path that would be written.
        path: PathBuf,
        /// The source it names.
        source: PathBuf,
    },
    /// The output, read back, does not carry the fingerprints of its sources, which the writer cannot
    /// cause and a person must know about.
    Mismatch {
        /// The path of the output.
        path: PathBuf,
        /// The track, by index in the output.
        track: usize,
    },
}

impl std::fmt::Display for MediaFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Open { path, cause } => write!(formatter, "cannot open {}: {cause}", path.display()),
            Self::Read { path, cause } => write!(formatter, "{}: {cause}", path.display()),
            Self::Manifest { path, cause } => {
                write!(
                    formatter,
                    "{} is not a manifest this version reads: {cause}",
                    path.display()
                )
            },
            Self::Write { path, cause } => write!(formatter, "cannot write {}: {cause}", path.display()),
            Self::Serialisation { cause } => write!(formatter, "cannot write the answer: {cause}"),
            Self::Selection { path, reason } => write!(formatter, "{}: {reason}", path.display()),
            Self::Export { path, cause } => write!(formatter, "{}: {cause}", path.display()),
            Self::Provenance { cause } => write!(formatter, "{cause}"),
            Self::Overwrite { path, source } => write!(
                formatter,
                "{} is the recording {} being exported; nothing was written",
                path.display(),
                source.display()
            ),
            Self::Mismatch { path, track } => write!(
                formatter,
                "{}: track {track} read back with a fingerprint that is not its source's; the output was \
                 removed",
                path.display()
            ),
        }
    }
}

impl std::error::Error for MediaFailure {}

/// How to print an answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// Prose for a person.
    Text,
    /// One JSON document for a pipeline.
    Json,
}

pub(crate) fn producer() -> Producer {
    Producer::current("adiungere", env!("CARGO_PKG_VERSION"))
}

pub(crate) fn json<T: serde::Serialize>(value: &T) -> Result<String, MediaFailure> {
    serde_json::to_string_pretty(value)
        .map(|text| format!("{text}\n"))
        .map_err(|cause| MediaFailure::Serialisation { cause })
}

pub(crate) fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn build_manifest(path: &Path, scope: Scope) -> Result<Manifest, MediaFailure> {
    let mut source = FileSource::open(path).map_err(|cause| MediaFailure::Open {
        path: path.to_path_buf(),
        cause,
    })?;
    let name = file_name(path);
    let file_name_time = parse_name(&name).and_then(|parsed| parsed.time);
    adiungere_manifest::build(&mut source, &name, scope, &producer(), file_name_time).map_err(|cause| {
        MediaFailure::Read {
            path: path.to_path_buf(),
            cause,
        }
    })
}

/// Reads a file's structure without touching its media, and describes it.
///
/// # Errors
///
/// Returns a failure when the file cannot be opened or read as a container.
pub fn inspect(path: &Path, output: Output) -> Result<Rendered, MediaFailure> {
    let manifest = build_manifest(path, Scope::Structure)?;
    let text = match output {
        Output::Json => manifest
            .to_canonical_json()
            .map_err(|cause| MediaFailure::Serialisation { cause })?,
        Output::Text => describe(&manifest),
    };
    Ok(Rendered {
        text,
        negative: false,
    })
}

/// Reads every byte of a file, computes every fingerprint, and writes the manifest.
///
/// # Errors
///
/// Returns a failure when the file cannot be read or the manifest cannot be written.
pub fn fingerprint(
    path: &Path,
    output: Output,
    manifest_to: Option<&Path>,
) -> Result<Rendered, MediaFailure> {
    let manifest = build_manifest(path, Scope::Full)?;
    let canonical = manifest
        .to_canonical_json()
        .map_err(|cause| MediaFailure::Serialisation { cause })?;

    if let Some(destination) = manifest_to {
        std::fs::write(destination, &canonical).map_err(|cause| MediaFailure::Write {
            path: destination.to_path_buf(),
            cause,
        })?;
    }

    let text = match output {
        Output::Json => canonical,
        Output::Text => describe(&manifest),
    };
    Ok(Rendered {
        text,
        negative: false,
    })
}

/// Scans paths for recordings.
///
/// # Errors
///
/// Returns a failure when the cache cannot be read or written, or the answer cannot be serialised.
pub fn detect(paths: &[PathBuf], output: Output, cache_at: Option<&Path>) -> Result<Rendered, MediaFailure> {
    let mut cache = match cache_at {
        Some(path) => Some(ScanCache::load(path).map_err(|cause| MediaFailure::Write {
            path: path.to_path_buf(),
            cause,
        })?),
        None => None,
    };

    let scanned = scan(paths, cache.as_mut());

    if let (Some(path), Some(cache)) = (cache_at, cache.as_ref()) {
        cache.save(path).map_err(|cause| MediaFailure::Write {
            path: path.to_path_buf(),
            cause,
        })?;
    }

    let negative = scanned
        .recordings
        .iter()
        .any(|recording| matches!(recording, Recording::Unreadable { .. }));

    let text = match output {
        Output::Json => json(&scanned)?,
        Output::Text => describe_recordings(&scanned),
    };
    Ok(Rendered { text, negative })
}

/// What `verify` found: the comparison, and the credentials and the time-stamp token when there are any.
#[derive(Debug, serde::Serialize)]
struct Verified {
    /// The comparison of the manifest with the file, subject by subject.
    comparison: Verification,
    /// The Content Credentials embedded in the file or beside it, when there are any.
    credentials: Option<crate::provenance::CredentialsCheck>,
    /// The time-stamp token beside the manifest, when there is one.
    token: Option<crate::provenance::TokenCheck>,
}

/// Compares a manifest with a file, then reads the Content Credentials the file carries and the time-stamp
/// token beside the manifest, when there are any. The answer is a no when any compared subject differs,
/// when the credentials do not hold, or when the token does not name the manifest.
///
/// # Errors
///
/// Returns a failure when the manifest, the file, the credentials or the token cannot be read.
pub fn verify(manifest_path: &Path, file: &Path, output: Output) -> Result<Rendered, MediaFailure> {
    let manifest = read_manifest(manifest_path)?;
    let mut source = FileSource::open(file).map_err(|cause| MediaFailure::Open {
        path: file.to_path_buf(),
        cause,
    })?;
    let comparison =
        adiungere_manifest::verify(&manifest, &mut source).map_err(|cause| MediaFailure::Read {
            path: file.to_path_buf(),
            cause,
        })?;
    drop(source);
    let credentials = crate::provenance::check_credentials(file, None)?;
    let token = crate::provenance::check_token(manifest_path)?;

    let negative = !comparison.every_compared_subject_is_identical()
        || credentials
            .as_ref()
            .is_some_and(|check| check.credentials.state == adiungere_provenance::State::Invalid)
        || token.as_ref().is_some_and(|check| check.mismatch.is_some());
    let verified = Verified {
        comparison,
        credentials,
        token,
    };
    let text = match output {
        Output::Json => json(&verified)?,
        Output::Text => describe_verified(&verified, &manifest),
    };
    Ok(Rendered { text, negative })
}

fn describe_verified(verified: &Verified, manifest: &Manifest) -> String {
    let mut text = describe_verification(&verified.comparison, manifest);
    let _ = writeln!(text);
    match &verified.credentials {
        Some(check) => {
            crate::provenance::describe_credentials(&mut text, &check.credentials, check.tracks_differing);
        },
        None => {
            let _ = writeln!(text, "  {}", render(Phrase::CredentialsNone, &[]));
        },
    }
    if let Some(check) = &verified.token {
        crate::provenance::describe_token(&mut text, check);
    }
    let _ = writeln!(text, "\n{}", render(Phrase::FactsOnly, &[]));
    text
}

/// Renders a manifest as a report a person can read and act on.
///
/// # Errors
///
/// Returns a failure when the manifest cannot be read.
pub fn report(manifest_path: &Path, output: Output) -> Result<Rendered, MediaFailure> {
    let manifest = read_manifest(manifest_path)?;
    let text = match output {
        Output::Json => manifest
            .to_canonical_json()
            .map_err(|cause| MediaFailure::Serialisation { cause })?,
        Output::Text => describe_report(&manifest),
    };
    Ok(Rendered {
        text,
        negative: false,
    })
}

/// Which tracks an export keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// The first video track and every audio track.
    Front,
    /// The second video track and every audio track.
    Rear,
    /// Every track.
    Both,
    /// Exactly these tracks of the first file, by index.
    Tracks(Vec<usize>),
}

/// What `export` writes besides the file: the report of the remux and the manifest of the output.
#[derive(Debug, serde::Serialize)]
struct Exported {
    /// Where the output was written.
    output: PathBuf,
    /// Where its manifest was written.
    manifest_path: PathBuf,
    /// What the remux did.
    report: ExportReport,
    /// The manifest of the output.
    manifest: Manifest,
    /// The credentials embedded in the output, when it was signed, read back and checked.
    credentials: Option<crate::provenance::CredentialsCheck>,
    /// Which kind of credential signed the output, when it was signed.
    credential: Option<adiungere_provenance::Kind>,
}

/// The remux report in the shape the answer carries.
#[derive(Debug, serde::Serialize)]
struct ExportReport {
    bytes_written: u64,
    chunks: u32,
    wide_offsets: bool,
    renumbered: bool,
    references_dropped: u32,
    tracks: Vec<ExportedTrack>,
    preserved: Vec<String>,
}

#[derive(Debug, serde::Serialize)]
struct ExportedTrack {
    input: usize,
    source_index: usize,
    output_index: usize,
    track_id: u32,
    source_track_id: u32,
    samples: u32,
    bytes: u64,
}

/// One source of an export, read in full before the output is written.
struct OpenSource {
    path: PathBuf,
    source: FileSource,
    container: adiungere_isobmff::Container,
    manifest: Manifest,
    tracks: Vec<usize>,
}

/// Writes one camera, both, or the chosen tracks of a recording into a new file without touching a
/// sample, and the manifest of the output beside it.
///
/// # Errors
///
/// Returns a failure when a source cannot be read, when the selection names tracks the source does not
/// have, when the output or its manifest cannot be written, or when the output read back does not carry
/// its sources' fingerprints.
pub fn export(
    files: &[PathBuf],
    selection: &Selection,
    out: &Path,
    manifest_to: Option<&Path>,
    signing: Option<&crate::provenance::Signing>,
    output: Output,
    stop: &AtomicBool,
) -> Result<Rendered, MediaFailure> {
    // The output is written under a temporary name and moved into place once it is complete and read
    // back, so a stopped or failed export never leaves a file that looks finished.
    let partial = out.with_extension("part");
    let manifest_path = manifest_to.map_or_else(|| manifest_beside(out), Path::to_path_buf);

    // Nothing this command writes may be a recording it reads, under any spelling of the path: the
    // check runs before a byte is written, and refuses the export rather than the source.
    for written in [out, &partial, &manifest_path] {
        if let Some(source) = files.iter().find(|file| same_file(file, written)) {
            return Err(MediaFailure::Overwrite {
                path: written.to_path_buf(),
                source: source.clone(),
            });
        }
    }

    // The credential is built before anything is written, so a credential that cannot be built stops
    // the export before it has cost a copy.
    let credential = signing.map(crate::provenance::Signing::credential).transpose()?;

    let mut sources = open_sources(files, selection, out)?;
    let class = class_of(&sources);
    let report = write_output(&mut sources, &partial, output, stop)?;
    let records = source_records(&sources, &report);
    let mut manifest = read_back(&partial, out, class, records.clone())?;
    if let Some(track) = track_not_carrying_its_source(&manifest) {
        let _ = std::fs::remove_file(&partial);
        return Err(MediaFailure::Mismatch {
            path: out.to_path_buf(),
            track,
        });
    }

    // Signing is the last step. The facts signed are the manifest of the file as written; the manifest
    // written beside the output then describes the signed file, whose bytes the embedded store changed.
    let credentials = if let Some(credential) = &credential {
        let request = adiungere_provenance::Request {
            asset: &partial,
            sources: files.iter().map(PathBuf::as_path).collect(),
            facts: &manifest,
            placement: adiungere_provenance::Placement::Embedded,
        };
        let signed = adiungere_provenance::sign(&request, credential, Some(out));
        let _ = std::fs::remove_file(&partial);
        signed.map_err(|cause| MediaFailure::Provenance {
            cause: cause.to_string(),
        })?;
        manifest = read_back(out, out, class, records)?;
        crate::provenance::check_credentials(out, None)?
    } else {
        std::fs::rename(&partial, out).map_err(|cause| MediaFailure::Write {
            path: out.to_path_buf(),
            cause,
        })?;
        None
    };

    let canonical = manifest
        .to_canonical_json()
        .map_err(|cause| MediaFailure::Serialisation { cause })?;
    std::fs::write(&manifest_path, &canonical).map_err(|cause| MediaFailure::Write {
        path: manifest_path.clone(),
        cause,
    })?;

    let exported = Exported {
        output: out.to_path_buf(),
        manifest_path,
        report: ExportReport {
            bytes_written: report.bytes_written,
            chunks: report.chunks,
            wide_offsets: report.wide_offsets,
            renumbered: report.renumbered,
            references_dropped: report.references_dropped,
            tracks: report
                .tracks
                .iter()
                .map(|track| ExportedTrack {
                    input: track.input,
                    source_index: track.source_index,
                    output_index: track.output_index,
                    track_id: track.track_id,
                    source_track_id: track.source_track_id,
                    samples: track.samples,
                    bytes: track.bytes,
                })
                .collect(),
            preserved: report
                .preserved
                .iter()
                .map(|preserved| preserved.kind.to_string())
                .collect(),
        },
        manifest,
        credentials,
        credential: credential.as_ref().map(adiungere_provenance::Credential::kind),
    };
    let text = match output {
        Output::Json => json(&exported)?,
        Output::Text => describe_export(&exported, &sources),
    };
    Ok(Rendered {
        text,
        negative: false,
    })
}

/// Opens one or two recordings and settles which tracks each contributes.
fn open_sources(
    files: &[PathBuf],
    selection: &Selection,
    out: &Path,
) -> Result<Vec<OpenSource>, MediaFailure> {
    let (first, second) = match files {
        [first] => (first, None),
        [first, second] => (first, Some(second)),
        _ => {
            return Err(MediaFailure::Selection {
                path: out.to_path_buf(),
                reason: "one or two recordings are exported at a time".to_owned(),
            });
        },
    };
    let mut sources = Vec::new();
    let mut primary = open_source(first)?;
    // In a join, the first file gives its front camera and its audio, the second its front camera as the
    // rear; a recorder that writes one file per camera writes the audio into both.
    primary.tracks = match second {
        Some(_) => choose(&primary, &Selection::Front)?,
        None => choose(&primary, selection)?,
    };
    sources.push(primary);
    if let Some(path) = second {
        if *selection != Selection::Both {
            return Err(MediaFailure::Selection {
                path: path.clone(),
                reason: "two recordings join into one two-track file, which keeps both cameras".to_owned(),
            });
        }
        let mut rear = open_source(path)?;
        rear.tracks = second_video_track(&rear)?;
        sources.push(rear);
    }
    Ok(sources)
}

/// The class an export belongs to: an archive when it carries more than one video track, an extraction
/// otherwise. The label derives from what is written and from nothing else.
fn class_of(sources: &[OpenSource]) -> ExportClass {
    let videos = sources
        .iter()
        .flat_map(|source| {
            source
                .tracks
                .iter()
                .filter_map(|&index| source.manifest.tracks.get(index))
        })
        .filter(|record| record.kind == TrackKind::Video)
        .count();
    if videos > 1 {
        ExportClass::TwoTrackArchive
    } else {
        ExportClass::Extraction
    }
}

/// The sources as the output's manifest records them: name, size, digest, and the fingerprint of every
/// track taken, with where it landed.
fn source_records(sources: &[OpenSource], report: &adiungere_isobmff::RemuxReport) -> Vec<SourceRecord> {
    sources
        .iter()
        .enumerate()
        .map(|(position, source)| SourceRecord {
            name: file_name(&source.path),
            size: source.manifest.file.size,
            sha256: source.manifest.file.sha256,
            tracks: source
                .tracks
                .iter()
                .map(|&index| SourceTrack {
                    index,
                    output_index: report
                        .tracks
                        .iter()
                        .find(|track| track.source_index == index && track.input == position)
                        .map_or(0, |track| track.output_index),
                    fingerprint: source
                        .manifest
                        .tracks
                        .get(index)
                        .and_then(|record| record.fingerprint.clone()),
                })
                .collect(),
        })
        .collect()
}

/// The first output track whose fingerprint, read back, is not the one its source had; none when the
/// output is what was meant.
fn track_not_carrying_its_source(manifest: &Manifest) -> Option<usize> {
    // What identifies the content: the samples and the decoder configuration. The position, the
    // identifier and the counts are the output's own.
    let identity = |f: &TrackFingerprint| (f.payload_sha256, f.configuration_sha256);
    manifest.tracks.iter().find_map(|track| {
        let written = track.fingerprint.as_ref().map(identity);
        let expected = manifest_source_fingerprint(manifest, track.index).map(identity);
        (written != expected).then_some(track.index)
    })
}

/// Whether two paths name one file, whatever their spelling: a relative form, a different case on a
/// system that ignores it, a short name, or a link. An existing file is resolved by the system; a file
/// not yet written is resolved through its directory, which has to exist for it to be written at all.
fn same_file(existing: &Path, written: &Path) -> bool {
    let Ok(existing) = std::fs::canonicalize(existing) else {
        return false;
    };
    let resolved = std::fs::canonicalize(written).or_else(|_| {
        let parent = written.parent().filter(|parent| !parent.as_os_str().is_empty());
        let directory = std::fs::canonicalize(parent.unwrap_or(Path::new(".")))?;
        written
            .file_name()
            .map(|name| directory.join(name))
            .ok_or_else(|| std::io::Error::other("no file name"))
    });
    resolved.is_ok_and(|resolved| resolved == existing)
}

/// The manifest's default place: beside the output, with the suffix appended to the whole file name.
pub(crate) fn manifest_beside(out: &Path) -> PathBuf {
    let mut name = out
        .file_name()
        .map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    name.push_str(".manifest.json");
    out.with_file_name(name)
}

/// Opens a recording, reads it in full for its manifest, and keeps it open for the copy.
fn open_source(path: &Path) -> Result<OpenSource, MediaFailure> {
    let mut source = FileSource::open(path).map_err(|cause| MediaFailure::Open {
        path: path.to_path_buf(),
        cause,
    })?;
    let name = file_name(path);
    let file_name_time = parse_name(&name).and_then(|parsed| parsed.time);
    let manifest = adiungere_manifest::build(&mut source, &name, Scope::Full, &producer(), file_name_time)
        .map_err(|cause| MediaFailure::Read {
            path: path.to_path_buf(),
            cause,
        })?;
    let container = adiungere_isobmff::parse(&mut source).map_err(|cause| MediaFailure::Read {
        path: path.to_path_buf(),
        cause: cause.into(),
    })?;
    Ok(OpenSource {
        path: path.to_path_buf(),
        source,
        container,
        manifest,
        tracks: Vec::new(),
    })
}

/// The track indices a selection names in a source.
fn choose(source: &OpenSource, selection: &Selection) -> Result<Vec<usize>, MediaFailure> {
    let videos: Vec<usize> = source
        .manifest
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Video)
        .map(|track| track.index)
        .collect();
    let audios: Vec<usize> = source
        .manifest
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Audio)
        .map(|track| track.index)
        .collect();
    let refuse = |reason: &str| MediaFailure::Selection {
        path: source.path.clone(),
        reason: reason.to_owned(),
    };
    let with_audio = |video: usize| {
        let mut tracks = vec![video];
        tracks.extend(audios.iter().copied());
        tracks.sort_unstable();
        tracks
    };
    match selection {
        Selection::Front => videos
            .first()
            .map(|&video| with_audio(video))
            .ok_or_else(|| refuse("the recording has no video track")),
        Selection::Rear => videos
            .get(1)
            .map(|&video| with_audio(video))
            .ok_or_else(|| refuse("the recording has one video track, so there is no rear camera to keep")),
        Selection::Both => Ok(source.manifest.tracks.iter().map(|track| track.index).collect()),
        Selection::Tracks(indices) => {
            if indices.is_empty() {
                return Err(refuse("no track was named"));
            }
            match indices
                .iter()
                .find(|&&index| index >= source.manifest.tracks.len())
            {
                Some(missing) => Err(refuse(&format!("the recording has no track {missing}"))),
                None => Ok(indices.clone()),
            }
        },
    }
}

/// The video track a rear-camera file contributes to a join: its first, and only its video.
fn second_video_track(source: &OpenSource) -> Result<Vec<usize>, MediaFailure> {
    source
        .manifest
        .tracks
        .iter()
        .find(|track| track.kind == TrackKind::Video)
        .map(|track| vec![track.index])
        .ok_or_else(|| MediaFailure::Selection {
            path: source.path.clone(),
            reason: "the second recording has no video track".to_owned(),
        })
}

/// Runs the remux into the partial file, reporting progress on the standard error stream for a person.
fn write_output(
    sources: &mut [OpenSource],
    partial: &Path,
    output: Output,
    stop: &AtomicBool,
) -> Result<adiungere_isobmff::RemuxReport, MediaFailure> {
    let file = std::fs::File::create(partial).map_err(|cause| MediaFailure::Write {
        path: partial.to_path_buf(),
        cause,
    })?;
    let mut writer = std::io::BufWriter::new(file);
    let mut inputs: Vec<adiungere_isobmff::Input<'_>> = sources
        .iter_mut()
        .map(|source| adiungere_isobmff::Input {
            container: &source.container,
            source: &mut source.source,
            tracks: source.tracks.clone(),
        })
        .collect();
    let mut last_percent = u64::MAX;
    let mut progress = |progress: &adiungere_isobmff::Progress| {
        if stop.load(Ordering::Relaxed) {
            return false;
        }
        if output == Output::Text && progress.bytes_total > 0 {
            let percent = progress
                .bytes_written
                .saturating_mul(100)
                .checked_div(progress.bytes_total)
                .unwrap_or(0);
            if percent != last_percent {
                eprint!("\r{percent:3}%");
                last_percent = percent;
            }
        }
        true
    };
    let outcome = adiungere_isobmff::remux(
        &mut inputs,
        &adiungere_isobmff::RemuxPlan::default(),
        &mut writer,
        &mut progress,
    );
    if output == Output::Text && last_percent != u64::MAX {
        eprintln!();
    }
    let report = match outcome {
        Ok(report) => report,
        Err(cause) => {
            drop(writer);
            let _ = std::fs::remove_file(partial);
            return Err(MediaFailure::Export {
                path: partial.to_path_buf(),
                cause,
            });
        },
    };
    writer.flush().map_err(|cause| MediaFailure::Write {
        path: partial.to_path_buf(),
        cause,
    })?;
    Ok(report)
}

/// Reads the written file back in full and builds its manifest, naming the export it came from.
fn read_back(
    partial: &Path,
    out: &Path,
    class: ExportClass,
    sources: Vec<SourceRecord>,
) -> Result<Manifest, MediaFailure> {
    let mut written = FileSource::open(partial).map_err(|cause| MediaFailure::Open {
        path: partial.to_path_buf(),
        cause,
    })?;
    let name = file_name(out);
    let file_name_time = parse_name(&name).and_then(|parsed| parsed.time);
    adiungere_manifest::build_for_export(
        &mut written,
        &name,
        &producer(),
        file_name_time,
        Operation::Export {
            class,
            masking: Masking::None,
            sources,
        },
    )
    .map_err(|cause| MediaFailure::Read {
        path: partial.to_path_buf(),
        cause,
    })
}

/// The fingerprint the sources recorded for the track that landed at an output index.
fn manifest_source_fingerprint(manifest: &Manifest, output_index: usize) -> Option<&TrackFingerprint> {
    match &manifest.produced.operation {
        Operation::Export { sources, .. } => sources
            .iter()
            .flat_map(|source| source.tracks.iter())
            .find(|track| track.output_index == output_index)
            .and_then(|track| track.fingerprint.as_ref()),
        Operation::Inspection { .. } => None,
    }
}

fn describe_export(exported: &Exported, sources: &[OpenSource]) -> String {
    let mut text = String::new();
    let class = match &exported.manifest.produced.operation {
        Operation::Export { class, .. } => class_label(*class),
        Operation::Inspection { .. } => "inspection",
    };
    let _ = writeln!(
        text,
        "{}",
        render(
            Phrase::ExportWritten,
            &[
                ("name", &file_name(&exported.output)),
                ("class", class),
                ("bytes", &exported.report.bytes_written.to_string()),
                ("count", &exported.report.tracks.len().to_string()),
            ]
        )
    );
    for track in &exported.report.tracks {
        let source_name = sources
            .get(track.input)
            .map_or_else(String::new, |source| file_name(&source.path));
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::ExportTrack,
                &[
                    ("output", &track.output_index.to_string()),
                    ("source", &track.source_index.to_string()),
                    ("name", &source_name),
                    ("samples", &track.samples.to_string()),
                    ("bytes", &track.bytes.to_string()),
                ]
            )
        );
    }
    if exported.report.preserved.is_empty() {
        let _ = writeln!(text, "  {}", render(Phrase::ExportPreservedNone, &[]));
    } else {
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::ExportPreserved,
                &[
                    ("count", &exported.report.preserved.len().to_string()),
                    ("list", &exported.report.preserved.join(", ")),
                ]
            )
        );
    }
    if exported.report.references_dropped > 0 {
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::ExportReferencesDropped,
                &[("count", &exported.report.references_dropped.to_string())]
            )
        );
    }
    let _ = writeln!(
        text,
        "  {}",
        render(
            Phrase::ExportManifest,
            &[("path", &exported.manifest_path.display().to_string())]
        )
    );
    if let Some(check) = &exported.credentials {
        crate::provenance::describe_credentials(&mut text, &check.credentials, check.tracks_differing);
        if exported.credential == Some(adiungere_provenance::Kind::Ephemeral) {
            let _ = writeln!(text, "  {}", render(Phrase::SignEphemeral, &[]));
        }
    }
    let _ = writeln!(text, "  {}", render(Phrase::ExportFaces, &[]));
    let _ = writeln!(text);
    let _ = writeln!(text, "{}", render(Phrase::FactsOnly, &[]));
    text
}

/// The permanent label of an export class, as the manifest writes it.
fn class_label(class: ExportClass) -> &'static str {
    match class {
        ExportClass::Extraction => "extraction",
        ExportClass::TwoTrackArchive => "two_track_archive",
        ExportClass::RenditionLossy => "rendition_lossy",
        ExportClass::RenditionLosslessVerified => "rendition_lossless_verified",
    }
}

fn read_manifest(path: &Path) -> Result<Manifest, MediaFailure> {
    let text = std::fs::read_to_string(path).map_err(|cause| MediaFailure::Manifest {
        path: path.to_path_buf(),
        cause: cause.to_string(),
    })?;
    Manifest::from_json(&text).map_err(|cause| MediaFailure::Manifest {
        path: path.to_path_buf(),
        cause: cause.to_string(),
    })
}

fn track_line(track: &adiungere_manifest::TrackRecord) -> String {
    let mut line = format!("{} ", track.index);
    match &track.kind {
        TrackKind::Video => {
            let _ = write!(
                line,
                "video {} {}x{}",
                track.codec_string.as_deref().unwrap_or(&track.coding),
                track.width.unwrap_or(0),
                track.height.unwrap_or(0)
            );
        },
        TrackKind::Audio => {
            let _ = write!(
                line,
                "audio {} {} Hz {} channel(s)",
                track.coding,
                track.sample_rate.unwrap_or(0),
                track.channels.unwrap_or(0)
            );
        },
        TrackKind::Other(kind) => {
            let _ = write!(line, "{kind} {}", track.coding);
        },
    }
    let _ = write!(line, ", {} samples", track.sample_count);
    if let Some(milliseconds) = track.duration_milliseconds {
        let _ = write!(
            line,
            ", {}.{:03} s",
            milliseconds.div_euclid(1000),
            milliseconds.rem_euclid(1000)
        );
    }
    if let Some(rate) = track.average_bit_rate {
        let _ = write!(
            line,
            ", {}.{:02} Mbit/s",
            rate.div_euclid(1_000_000),
            rate.rem_euclid(1_000_000).div_euclid(10_000)
        );
    }
    if !track.enabled {
        line.push_str(", disabled");
    }
    line
}

fn describe(manifest: &Manifest) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "{}  {} bytes", manifest.file.name, manifest.file.size);

    let structure = &manifest.file.structure;
    let _ = writeln!(
        text,
        "  brand {}{}; top level: {}",
        structure.brand.as_deref().unwrap_or("none"),
        if structure.compatible_brands.is_empty() {
            String::new()
        } else {
            format!(" ({})", structure.compatible_brands.join(" "))
        },
        structure.top_level.join(" ")
    );
    let moov = match structure.moov_before_mdat {
        Some(true) => Phrase::MoovFirst,
        Some(false) => Phrase::MoovLast,
        None => Phrase::MoovNone,
    };
    let _ = writeln!(text, "  {}", render(moov, &[]));
    if let Some(bytes) = structure.clipped_tail_bytes {
        let _ = writeln!(
            text,
            "  {}",
            render(Phrase::ClippedTail, &[("bytes", &bytes.to_string())])
        );
    }

    let list: Vec<String> = manifest.tracks.iter().map(track_line).collect();
    let _ = writeln!(
        text,
        "  {}",
        render(
            Phrase::Tracks,
            &[
                ("count", &manifest.tracks.len().to_string()),
                ("list", &list.join("; "))
            ]
        )
    );

    let default_video = manifest
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Video && track.enabled)
        .count();
    if default_video > 1 {
        let _ = writeln!(
            text,
            "  {}",
            render(Phrase::DefaultTracks, &[("count", &default_video.to_string())])
        );
    }

    let describe_box = |vendor: &VendorBox| {
        format!(
            "{} ({} bytes {})",
            vendor.kind,
            vendor.size,
            match vendor.location {
                VendorLocation::UserData => "under user data",
                VendorLocation::TopLevel => "at the top level",
            }
        )
    };
    let (standard, vendor): (Vec<_>, Vec<_>) =
        manifest.vendor.boxes.iter().partition(|vendor| vendor.standard);
    if vendor.is_empty() {
        let _ = writeln!(text, "  {}", render(Phrase::VendorAbsent, &[]));
    } else {
        let list: Vec<String> = vendor.iter().map(|vendor| describe_box(vendor)).collect();
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::VendorPresent,
                &[("count", &vendor.len().to_string()), ("list", &list.join(", "))]
            )
        );
    }
    if !standard.is_empty() {
        let list: Vec<String> = standard.iter().map(|vendor| describe_box(vendor)).collect();
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::StandardUserData,
                &[("count", &standard.len().to_string()), ("list", &list.join(", "))]
            )
        );
    }

    describe_times(&mut text, manifest);
    describe_fingerprints(&mut text, manifest);

    let _ = writeln!(text, "\n{}", render(Phrase::FactsOnly, &[]));
    text
}

fn describe_times(text: &mut String, manifest: &Manifest) {
    for time in &manifest.times {
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::NoLaterThan,
                &[("value", &time.value), ("clock", time.source.describe())]
            )
        );
    }
    if manifest.times.len() > 1 {
        let list: Vec<String> = manifest
            .times
            .iter()
            .map(|time| format!("{} says {}", time.source.describe(), time.value))
            .collect();
        let _ = writeln!(
            text,
            "  {}",
            render(Phrase::TimeDisagreement, &[("list", &list.join(", "))])
        );
    }
}

fn describe_fingerprints(text: &mut String, manifest: &Manifest) {
    let structure = &manifest.file.structure;
    for track in &manifest.tracks {
        if let Some(fingerprint) = &track.fingerprint {
            let _ = writeln!(
                text,
                "  {}",
                render(
                    Phrase::FingerprintTrack,
                    &[
                        ("index", &track.index.to_string()),
                        ("version", &fingerprint.version),
                        ("digest", &fingerprint.payload_sha256.to_string()),
                        ("count", &fingerprint.sample_count.to_string()),
                        ("bytes", &fingerprint.sample_bytes.to_string()),
                        (
                            "width",
                            &fingerprint
                                .nal_length_size
                                .map_or_else(|| "none".to_owned(), |width| width.to_string())
                        ),
                    ]
                )
            );
            let _ = writeln!(
                text,
                "  {}",
                render(
                    Phrase::FingerprintConfiguration,
                    &[
                        ("index", &track.index.to_string()),
                        ("box", &fingerprint.configuration_box),
                        ("digest", &fingerprint.configuration_sha256.to_string()),
                    ]
                )
            );
        }
        if let Some(stream) = &track.elementary_stream {
            let _ = writeln!(
                text,
                "  {}",
                render(
                    Phrase::FingerprintStream,
                    &[
                        ("index", &track.index.to_string()),
                        ("rule", &stream.rule),
                        ("digest", &stream.sha256.to_string()),
                        ("bytes", &stream.stream_bytes.to_string()),
                    ]
                )
            );
        }
    }
    if let Some(digest) = manifest.file.sha256 {
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::FingerprintFile,
                &[
                    ("digest", &digest.to_string()),
                    ("bytes", &manifest.file.size.to_string())
                ]
            )
        );
    }
    let _ = writeln!(
        text,
        "  {}",
        render(
            Phrase::FingerprintStructure,
            &[
                ("digest", &structure.structural_fingerprint.sha256.to_string()),
                (
                    "count",
                    &structure.structural_fingerprint.observations.len().to_string()
                ),
            ]
        )
    );
}

fn describe_origin(text: &mut String, origin: &adiungere_scan::Origin) {
    if origin.signals.is_empty() {
        if origin.layout_known {
            let _ = writeln!(text, "    {}", render(Phrase::RewriteNone, &[]));
        } else {
            let _ = writeln!(text, "    {}", render(Phrase::UnknownLayout, &[]));
        }
        return;
    }
    let _ = writeln!(
        text,
        "    {}",
        render(
            Phrase::RewriteSome,
            &[("count", &origin.signals.len().to_string())]
        )
    );
    for signal in &origin.signals {
        let line = match signal {
            Signal::MoovAfterMdat => render(Phrase::SignMoovAfterMdat, &[]),
            Signal::ForeignMuxerTag { tag } => render(Phrase::SignEncoderTag, &[("tag", tag)]),
            Signal::SingleVideoTrackWhereSiblingHasTwo => render(Phrase::SignSingleVideo, &[]),
            Signal::VendorBoxesAbsent => render(Phrase::SignVendorAbsent, &[]),
            Signal::ResolutionBelowSiblings { width, height, .. } => render(
                Phrase::SignResolutionCapped,
                &[("width", &width.to_string()), ("height", &height.to_string())],
            ),
        };
        let _ = writeln!(text, "      {line}");
    }
}

/// Describes a pair: the two files named, then what each one's structure shows.
fn describe_pair(text: &mut String, members: &[adiungere_scan::Member]) {
    let path_of = |camera: adiungere_scan::Camera, absent: &str| {
        members.iter().find(|member| member.camera == camera).map_or_else(
            || absent.to_owned(),
            |member| member.file.path.display().to_string(),
        )
    };
    let front = path_of(adiungere_scan::Camera::Front, "no front file");
    let rear = path_of(adiungere_scan::Camera::Rear, "no rear file");
    let _ = writeln!(
        text,
        "  {}",
        render(Phrase::ScanPaired, &[("front", &front), ("rear", &rear)])
    );
    for member in members {
        describe_origin(text, &member.origin);
    }
}

/// Describes one recording: what it is, then what its structure shows about where it came from.
fn describe_recording(text: &mut String, recording: &Recording) {
    match recording {
        Recording::SingleFile {
            file,
            video_tracks,
            origin,
        } => {
            let phrase = if *video_tracks >= 2 {
                Phrase::ScanSingleDual
            } else {
                Phrase::ScanSingleMono
            };
            let _ = writeln!(
                text,
                "  {}",
                render(phrase, &[("name", &file.path.display().to_string())])
            );
            describe_origin(text, origin);
        },
        Recording::Paired { members, .. } => describe_pair(text, members),
        Recording::Unpaired { file, origin, .. } => {
            let _ = writeln!(
                text,
                "  {}",
                render(
                    Phrase::ScanUnpaired,
                    &[("name", &file.path.display().to_string())]
                )
            );
            describe_origin(text, origin);
        },
        Recording::Unrecognised {
            file,
            video_tracks,
            origin,
        } => {
            let _ = writeln!(
                text,
                "  {}",
                render(
                    Phrase::ScanNotRecognised,
                    &[
                        ("name", &file.path.display().to_string()),
                        ("count", &video_tracks.to_string()),
                    ]
                )
            );
            describe_origin(text, origin);
        },
        Recording::Unreadable { file, reason } => {
            let _ = writeln!(
                text,
                "  {}",
                render(
                    Phrase::ScanNotReadable,
                    &[("name", &file.path.display().to_string()), ("reason", reason)]
                )
            );
        },
    }
}

fn describe_recordings(scanned: &Scan) -> String {
    let recordings = &scanned.recordings;
    let mut text = String::new();
    for recording in recordings {
        describe_recording(&mut text, recording);
    }
    if recordings.is_empty() {
        let _ = writeln!(text, "  {}", render(Phrase::ScanNothing, &[]));
    }
    if scanned.skipped_by_extension > 0 {
        let _ = writeln!(
            text,
            "  {}",
            render(
                Phrase::ScanSkipped,
                &[("count", &scanned.skipped_by_extension.to_string())]
            )
        );
    }
    let _ = writeln!(text, "\n{}", render(Phrase::NotAnExamination, &[]));
    text
}

fn describe_verification(verification: &Verification, manifest: &Manifest) -> String {
    let mut text = String::new();
    let _ = writeln!(text, "Compared with the manifest for {}:", manifest.file.name);

    for finding in &verification.findings {
        let line = match (&finding.subject, &finding.outcome) {
            (Subject::WholeFile, Outcome::Identical) => render(Phrase::FileIdentical, &[]),
            (Subject::WholeFile, Outcome::Differs { recorded, observed }) => render(
                Phrase::FileDiffers,
                &[
                    ("recorded", &recorded.to_string()),
                    ("observed", &observed.to_string()),
                ],
            ),
            (Subject::WholeFile, _) => render(Phrase::FileNotRecorded, &[]),
            (Subject::Track { index }, outcome) => {
                let index = index.to_string();
                match outcome {
                    Outcome::Identical => render(Phrase::TrackIdentical, &[("index", &index)]),
                    Outcome::Differs { .. } => render(Phrase::TrackDiffers, &[("index", &index)]),
                    Outcome::NotRecorded | Outcome::Unexpected => {
                        render(Phrase::TrackNotRecorded, &[("index", &index)])
                    },
                    Outcome::Missing => render(Phrase::TrackMissing, &[("index", &index)]),
                    Outcome::NotCheckable { reason } => render(
                        Phrase::TrackNotCheckable,
                        &[("index", &index), ("reason", reason)],
                    ),
                }
            },
            (Subject::VendorBox { location, kind }, outcome) => {
                let location = match location {
                    VendorLocation::UserData => "user data",
                    VendorLocation::TopLevel => "the top level",
                };
                let values = [("kind", kind.as_str()), ("location", location)];
                match outcome {
                    Outcome::Identical => render(Phrase::VendorIdentical, &values),
                    Outcome::Differs { .. } => render(Phrase::VendorDiffers, &values),
                    Outcome::Missing => render(Phrase::VendorMissing, &values),
                    Outcome::Unexpected => render(Phrase::VendorUnexpected, &values),
                    Outcome::NotRecorded | Outcome::NotCheckable { .. } => {
                        render(Phrase::VendorMissing, &values)
                    },
                }
            },
            (Subject::Structure, Outcome::Identical) => render(Phrase::StructureIdentical, &[]),
            (Subject::Structure, _) => render(Phrase::StructureDiffers, &[]),
        };
        let _ = writeln!(text, "  {line}");
    }
    text
}

fn describe_report(manifest: &Manifest) -> String {
    let mut text = describe(manifest);
    text.push('\n');
    text.push_str("How to reproduce every number above without this program:\n");
    let _ = writeln!(text, "  whole file: sha256sum {}", manifest.file.name);
    for track in &manifest.tracks {
        if track.fingerprint.is_some() {
            let _ = writeln!(
                text,
                "  track {} fingerprint: python3 scripts/track-fingerprint.py {} {}",
                track.index, manifest.file.name, track.index
            );
        }
        if track.elementary_stream.is_some() {
            let video_index = manifest
                .tracks
                .iter()
                .filter(|other| other.kind == TrackKind::Video && other.index < track.index)
                .count();
            let _ = writeln!(
                text,
                "  track {} elementary stream: ffmpeg -v error -i {} -map 0:v:{video_index} -c copy -f h264 - | sha256sum",
                track.index, manifest.file.name
            );
        }
    }
    let _ = writeln!(
        text,
        "  produced by {} {} on {} at {}",
        manifest.produced.tool,
        manifest.produced.version,
        manifest.produced.platform,
        manifest.produced.at.value
    );
    text
}
