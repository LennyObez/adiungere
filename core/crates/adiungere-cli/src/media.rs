//! The recording commands: inspect, fingerprint, detect, verify, report.
//!
//! Each command answers with text for a person or one JSON document for a pipeline, and a status: 0 when
//! the answer is a yes, 1 when it is a no, 2 when the question could not be asked. A "no" only exists for
//! `verify`, where it means a compared subject differs, and for `detect`, where it means a file could not
//! be read; neither is a judgement about a recording, both are facts about bytes.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use adiungere_isobmff::FileSource;
use adiungere_manifest::wording::{Phrase, render};
use adiungere_manifest::{
    Manifest, Outcome, Producer, Scope, Subject, TrackKind, VendorBox, VendorLocation, Verification,
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

fn producer() -> Producer {
    Producer::current("adiungere", env!("CARGO_PKG_VERSION"))
}

fn json<T: serde::Serialize>(value: &T) -> Result<String, MediaFailure> {
    serde_json::to_string_pretty(value)
        .map(|text| format!("{text}\n"))
        .map_err(|cause| MediaFailure::Serialisation { cause })
}

fn file_name(path: &Path) -> String {
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

/// Compares a manifest with a file. The answer is a no when any compared subject differs.
///
/// # Errors
///
/// Returns a failure when the manifest or the file cannot be read.
pub fn verify(manifest_path: &Path, file: &Path, output: Output) -> Result<Rendered, MediaFailure> {
    let manifest = read_manifest(manifest_path)?;
    let mut source = FileSource::open(file).map_err(|cause| MediaFailure::Open {
        path: file.to_path_buf(),
        cause,
    })?;
    let verification =
        adiungere_manifest::verify(&manifest, &mut source).map_err(|cause| MediaFailure::Read {
            path: file.to_path_buf(),
            cause,
        })?;

    let negative = !verification.every_compared_subject_is_identical();
    let text = match output {
        Output::Json => json(&verification)?,
        Output::Text => describe_verification(&verification, &manifest),
    };
    Ok(Rendered { text, negative })
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

fn describe_recordings(scanned: &Scan) -> String {
    let recordings = &scanned.recordings;
    let mut text = String::new();
    for recording in recordings {
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
                describe_origin(&mut text, origin);
            },
            Recording::Paired { members, .. } => {
                let front = members
                    .iter()
                    .find(|member| member.camera == adiungere_scan::Camera::Front)
                    .map_or_else(
                        || "no front file".to_owned(),
                        |member| member.file.path.display().to_string(),
                    );
                let rear = members
                    .iter()
                    .find(|member| member.camera == adiungere_scan::Camera::Rear)
                    .map_or_else(
                        || "no rear file".to_owned(),
                        |member| member.file.path.display().to_string(),
                    );
                let _ = writeln!(
                    text,
                    "  {}",
                    render(Phrase::ScanPaired, &[("front", &front), ("rear", &rear)])
                );
                for member in members {
                    describe_origin(&mut text, &member.origin);
                }
            },
            Recording::Unpaired { file, origin, .. } => {
                let _ = writeln!(
                    text,
                    "  {}",
                    render(
                        Phrase::ScanUnpaired,
                        &[("name", &file.path.display().to_string())]
                    )
                );
                describe_origin(&mut text, origin);
            },
            Recording::Unrecognised { file } => {
                let _ = writeln!(
                    text,
                    "  {}",
                    render(
                        Phrase::ScanNotRecognised,
                        &[("name", &file.path.display().to_string())]
                    )
                );
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
    if recordings.is_empty() {
        text.push_str("  Nothing was found.\n");
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

    let _ = writeln!(text, "\n{}", render(Phrase::FactsOnly, &[]));
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
