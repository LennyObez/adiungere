//! Building a manifest from a file.

use adiungere_fingerprint::{annex_b_digest, file_digest, structural_fingerprint, track_fingerprint};
use adiungere_isobmff::{Container, Source, Track, TrackKind as ContainerTrackKind, parse};

use crate::Error;
use crate::manifest::{
    EditEntry, FileRecord, MANIFEST_FORMAT, Manifest, Operation, Produced, Scope, Structure, TrackKind,
    TrackRecord, VendorBox, VendorLocation, VendorRecord,
};
use crate::time::{Civil, TimeSource, TimeValue, now_utc};
use crate::wording::{Phrase, render};

/// Who is producing the manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Producer {
    /// The tool's name.
    pub tool: String,
    /// The tool's version.
    pub version: String,
    /// The operating system and architecture.
    pub platform: String,
}

impl Producer {
    /// The producer for the current build, on the current platform.
    #[must_use]
    pub fn current(tool: &str, version: &str) -> Self {
        Self {
            tool: tool.to_owned(),
            version: version.to_owned(),
            platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        }
    }
}

/// Reads a file and builds its manifest.
///
/// With [`Scope::Structure`], only the headers and the movie box are read and no fingerprint is computed.
/// With [`Scope::Full`], every sample is read for the fingerprints and every byte for the file digest.
///
/// # Errors
///
/// Returns an error when the file cannot be read as a container, or a fingerprint cannot be computed.
pub fn build<S: Source>(
    source: &mut S,
    name: &str,
    scope: Scope,
    producer: &Producer,
    file_name_time: Option<Civil>,
) -> Result<Manifest, Error> {
    let container = parse(source)?;
    let tracks = container.tracks()?;

    let structure = structure_of(&container, &tracks);
    let sha256 = match scope {
        Scope::Structure => None,
        Scope::Full => Some(file_digest(source)?.0),
    };

    let mut records = Vec::with_capacity(tracks.len());
    for track in &tracks {
        records.push(track_record(source, &container, track, scope)?);
    }

    let times = times_of(&container, file_name_time)?;

    Ok(Manifest {
        format: MANIFEST_FORMAT.to_owned(),
        produced: Produced {
            tool: producer.tool.clone(),
            version: producer.version.clone(),
            platform: producer.platform.clone(),
            at: TimeValue {
                value: now_utc().utc(),
                source: TimeSource::SystemClockAtManifest,
                note: render(Phrase::SystemClockNote, &[]),
            },
            operation: Operation::Inspection { scope },
        },
        file: FileRecord {
            name: name.to_owned(),
            size: container.length(),
            sha256,
            structure,
        },
        tracks: records,
        vendor: vendor_record(&container),
        times,
    })
}

fn structure_of(container: &Container, tracks: &[Track]) -> Structure {
    let ftyp = container.ftyp();
    let moov = container.moov();
    let mut encoder_tags: Vec<String> = Vec::new();
    for track in tracks {
        let compressor = track
            .entry()
            .and_then(|entry| entry.visual.as_ref())
            .map(|visual| visual.compressor_name.clone())
            .filter(|name| !name.is_empty());
        for tag in compressor
            .into_iter()
            .chain(std::iter::once(track.hdlr.name.clone()))
        {
            if !tag.is_empty() && !encoder_tags.contains(&tag) {
                encoder_tags.push(tag);
            }
        }
    }

    Structure {
        brand: ftyp.as_ref().map(|ftyp| ftyp.major_brand.to_string()),
        compatible_brands: ftyp.as_ref().map_or_else(Vec::new, |ftyp| {
            ftyp.compatible_brands.iter().map(ToString::to_string).collect()
        }),
        top_level: container
            .top_level()
            .iter()
            .map(|range| range.kind.to_string())
            .collect(),
        moov_before_mdat: container.moov_before_mdat(),
        moov_offset: moov.offset,
        moov_size: moov.size,
        encoder_tags,
        clipped_tail_bytes: container.clipped_tail().map(|tail| tail.size),
        structural_fingerprint: structural_fingerprint(container),
    }
}

fn track_record<S: Source>(
    source: &mut S,
    container: &Container,
    track: &Track,
    scope: Scope,
) -> Result<TrackRecord, Error> {
    let entry = track.entry();
    let kind = match track.kind {
        ContainerTrackKind::Video => TrackKind::Video,
        ContainerTrackKind::Audio => TrackKind::Audio,
        ContainerTrackKind::Other(code) => TrackKind::Other(code.to_string()),
    };

    let (fingerprint, elementary_stream) = match scope {
        Scope::Structure => (None, None),
        Scope::Full => {
            let fingerprint = track_fingerprint(source, container, track)?;
            let stream = if entry
                .and_then(|entry| entry.avc_configuration(container))
                .is_some()
            {
                Some(annex_b_digest(source, container, track)?)
            } else {
                None
            };
            (Some(fingerprint), stream)
        },
    };

    let duration_milliseconds = track.duration_milliseconds();
    let average_bit_rate = duration_milliseconds.and_then(|milliseconds| {
        let bits = u128::from(track.table.total_sample_bytes()).checked_mul(8_000)?;
        u64::try_from(bits.checked_div(u128::from(milliseconds))?).ok()
    });

    Ok(TrackRecord {
        index: track.index,
        id: track.tkhd.track_id,
        kind,
        coding: entry.map_or_else(|| "none".to_owned(), |entry| entry.kind.to_string()),
        codec_string: entry.and_then(|entry| entry.codec_string(container)),
        width: entry
            .and_then(|entry| entry.visual.as_ref())
            .map(|visual| visual.width),
        height: entry
            .and_then(|entry| entry.visual.as_ref())
            .map(|visual| visual.height),
        sample_rate: entry
            .and_then(|entry| entry.audio.as_ref())
            .map(|audio| audio.sample_rate),
        channels: entry
            .and_then(|entry| entry.audio.as_ref())
            .map(|audio| audio.channel_count),
        timescale: track.mdhd.timescale,
        duration_ticks: track.mdhd.duration,
        duration_milliseconds,
        sample_count: track.table.sample_count(),
        sync_sample_count: track.table.sync_sample_count(),
        composition_offsets: track.table.has_composition_offsets(),
        flags: track.tkhd.flags,
        enabled: track.tkhd.is_enabled(),
        edit_list: track.elst.as_ref().map(|elst| {
            elst.entries
                .iter()
                .map(|(segment_duration, media_time, media_rate)| EditEntry {
                    segment_duration: *segment_duration,
                    media_time: *media_time,
                    media_rate: *media_rate,
                })
                .collect()
        }),
        average_bit_rate,
        fingerprint,
        elementary_stream,
    })
}

fn vendor_record(container: &Container) -> VendorRecord {
    let mut boxes = Vec::new();

    for range in container.udta_children() {
        boxes.push(VendorBox {
            location: VendorLocation::UserData,
            kind: range.kind.to_string(),
            offset: range.offset,
            size: range.size,
            standard: range.kind.is_standard_user_data(),
            sha256: container.bytes_of(range).map(adiungere_fingerprint::Digest::of),
        });
    }

    for range in container.unknown_top_level() {
        boxes.push(VendorBox {
            location: VendorLocation::TopLevel,
            kind: range.kind.to_string(),
            offset: range.offset,
            size: range.size,
            standard: false,
            sha256: container.bytes_of(range).map(adiungere_fingerprint::Digest::of),
        });
    }

    boxes.sort_by_key(|vendor| vendor.offset);

    VendorRecord {
        boxes,
        interpreted: false,
    }
}

fn times_of(container: &Container, file_name_time: Option<Civil>) -> Result<Vec<TimeValue>, Error> {
    let mvhd = container.mvhd()?;
    let mut times = vec![TimeValue {
        value: Civil::from_container_seconds(mvhd.creation_time).unzoned(),
        source: TimeSource::ContainerClock,
        note: render(Phrase::ContainerClockNote, &[]),
    }];

    if let Some(civil) = file_name_time {
        times.push(TimeValue {
            value: civil.unzoned(),
            source: TimeSource::FileName,
            note: render(Phrase::FileNameNote, &[]),
        });
    }

    Ok(times)
}
