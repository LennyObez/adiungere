//! The public synthetic recordings the test suite runs on.
//!
//! The reference recording is private: it carries satellite positions and a route. So the required checks
//! run on recordings built here, from three small committed streams, that reproduce every structural
//! property the reference has: two video tracks in the same coding, both marked as default, one audio
//! track, a vendor box of thirty kilobytes under the user-data box, a vendor box of twelve bytes at the top
//! level, the movie box first, and four-byte length prefixes on every unit. Variants change one property
//! at a time: the movie box last, narrower prefixes, an edit list, an unknown child inside a sample entry,
//! sixty-four-bit chunk offsets, and a file whose media lies beyond four gigabytes, laid out sparsely.
//!
//! The builder links no encoder. The frames were encoded once, by the maintainer, with the commands
//! recorded next to them, and are committed as they were produced.

#![forbid(unsafe_code)]

pub mod frames;
mod mux;

use std::io::{Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use adiungere_isobmff::{Source, SourceError};

use frames::{AudioStream, FrameError, VideoStream};
use mux::{Fields, boxed, container, descriptor, full_box, header, large_header};

/// Seconds between the container epoch (1904) and a fixed synthetic instant, 2026-01-01T00:00:00Z.
pub const CREATION_TIME: u64 = 2_082_844_800 + 1_767_225_600;
/// The movie timescale, as the reference recording uses.
pub const MOVIE_TIMESCALE: u32 = 60000;
/// The video media timescale.
pub const VIDEO_TIMESCALE: u32 = 30000;
/// The duration of one video sample in its timescale: thirty frames per second.
pub const VIDEO_SAMPLE_DURATION: u32 = 1000;
/// The duration of one audio frame in its timescale.
pub const AUDIO_SAMPLE_DURATION: u32 = 1024;
/// The size of the vendor box under the user-data box, header included, as the reference has.
pub const VENDOR_BOX_SIZE: usize = 30720;
/// The type of the synthetic vendor box under the user-data box. Unknown to every parser on purpose.
pub const VENDOR_BOX: [u8; 4] = *b"zvnd";
/// The type of the synthetic vendor box at the top level.
pub const TOP_LEVEL_BOX: [u8; 4] = *b"zzzz";
/// The four-byte payload of the top-level vendor box: a model code, as the reference carries.
pub const TOP_LEVEL_PAYLOAD: [u8; 4] = *b"6350";
/// The type of the unknown child placed inside a video sample entry by one variant.
pub const ENTRY_CHILD_BOX: [u8; 4] = *b"zunk";
/// Where the media begins in the sparse variant: past four gigabytes.
pub const SPARSE_GAP: u64 = (4 << 30) + 4096;
/// The coded width and height of the synthetic frames.
pub const FRAME_SIZE: u16 = 64;

/// How the boxes are laid out in the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Layout {
    /// Whether the movie box precedes the media data box.
    pub moov_first: bool,
    /// Whether chunk offsets use the sixty-four-bit table regardless of need.
    pub wide_offsets: bool,
    /// Whether the media is placed beyond four gigabytes, with the gap left as a hole.
    pub sparse: bool,
}

/// Which tracks the recording carries beyond the front camera, which is always present.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tracks {
    /// Whether the rear camera track is present.
    pub rear: bool,
    /// Whether the audio track is present.
    pub audio: bool,
}

/// Which vendor boxes the recording carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Vendor {
    /// Whether the user-data box carries the synthetic vendor box.
    pub udta_box: bool,
    /// Whether the top level carries the twelve-byte synthetic vendor box after the movie box.
    pub top_level_box: bool,
}

/// Structural oddities a variant introduces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quirks {
    /// Whether each track carries an edit list with a non-zero media time.
    pub edit_list: bool,
    /// Whether the front video sample entry carries an unknown child box after its configuration.
    pub unknown_child_in_entry: bool,
}

/// How samples are grouped into chunks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunking {
    /// How many video samples share one chunk.
    pub video: u32,
    /// How many audio frames share one chunk.
    pub audio: u32,
}

/// What one synthetic recording looks like.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spec {
    /// A short name, used as the file name.
    pub name: &'static str,
    /// The box layout.
    pub layout: Layout,
    /// The length prefix width on every video unit: 1, 2 or 4.
    pub nal_length_size: u8,
    /// The tracks present.
    pub tracks: Tracks,
    /// The vendor boxes present.
    pub vendor: Vendor,
    /// The oddities introduced.
    pub quirks: Quirks,
    /// The compressor name written into each video sample entry.
    pub compressor: &'static str,
    /// The chunk grouping.
    pub chunking: Chunking,
}

impl Spec {
    /// The recording that mirrors the reference: every property, nothing unusual.
    #[must_use]
    pub const fn reference_like() -> Self {
        Self {
            name: "reference-like",
            layout: Layout {
                moov_first: true,
                wide_offsets: false,
                sparse: false,
            },
            nal_length_size: 4,
            tracks: Tracks {
                rear: true,
                audio: true,
            },
            vendor: Vendor {
                udta_box: true,
                top_level_box: true,
            },
            quirks: Quirks {
                edit_list: false,
                unknown_child_in_entry: false,
            },
            compressor: "synthetic pattern",
            chunking: Chunking { video: 15, audio: 22 },
        }
    }

    /// Whether the chunk offsets are written in the sixty-four-bit table.
    #[must_use]
    pub const fn wide_offsets(&self) -> bool {
        self.layout.wide_offsets || self.layout.sparse
    }
}

/// Every recording the corpus holds, each changing one thing from the reference-like one.
#[must_use]
pub fn corpus() -> Vec<Spec> {
    let base = Spec::reference_like();
    vec![
        base.clone(),
        Spec {
            name: "moov-last",
            layout: Layout {
                moov_first: false,
                ..base.layout
            },
            ..base.clone()
        },
        Spec {
            name: "nal-length-1",
            nal_length_size: 1,
            ..base.clone()
        },
        Spec {
            name: "nal-length-2",
            nal_length_size: 2,
            ..base.clone()
        },
        Spec {
            name: "edit-list",
            quirks: Quirks {
                edit_list: true,
                ..base.quirks
            },
            ..base.clone()
        },
        Spec {
            name: "unknown-child-in-entry",
            quirks: Quirks {
                unknown_child_in_entry: true,
                ..base.quirks
            },
            ..base.clone()
        },
        Spec {
            name: "wide-offsets",
            layout: Layout {
                wide_offsets: true,
                ..base.layout
            },
            ..base.clone()
        },
        Spec {
            name: "beyond-4gib",
            layout: Layout {
                sparse: true,
                ..base.layout
            },
            ..base.clone()
        },
        Spec {
            name: "rewritten",
            layout: Layout {
                moov_first: false,
                ..base.layout
            },
            tracks: Tracks {
                rear: false,
                ..base.tracks
            },
            vendor: Vendor {
                udta_box: false,
                top_level_box: false,
            },
            compressor: "another program",
            ..base.clone()
        },
        Spec {
            name: "one-sample-per-chunk",
            chunking: Chunking { video: 1, audio: 1 },
            ..base
        },
    ]
}

/// Which camera or stream a track carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// The front camera.
    Front,
    /// The rear camera.
    Rear,
    /// The audio.
    Audio,
}

/// What a test can expect a reader to find in one track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpectedTrack {
    /// Which stream.
    pub role: Role,
    /// The track identifier written into the header.
    pub track_id: u32,
    /// Every sample exactly as stored, in decode order.
    pub samples: Vec<Vec<u8>>,
    /// One-based numbers of the sync samples, or every sample for audio.
    pub sync_samples: Vec<u32>,
    /// The payload of the decoder configuration box, as written.
    pub configuration_payload: Vec<u8>,
    /// The sequence parameter sets, for video.
    pub sequence_parameter_sets: Vec<Vec<u8>>,
    /// The picture parameter sets, for video.
    pub picture_parameter_sets: Vec<Vec<u8>>,
}

/// Everything a test can expect a reader to find.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expected {
    /// The tracks in file order.
    pub tracks: Vec<ExpectedTrack>,
    /// The vendor box under the user-data box, header included, when present.
    pub vendor_box: Option<Vec<u8>>,
    /// The top-level vendor box, header included, when present.
    pub top_level_box: Option<Vec<u8>>,
    /// The absolute offset of the media data box.
    pub mdat_offset: u64,
    /// The size of the media data box, header included.
    pub mdat_size: u64,
    /// The absolute offset of the movie box.
    pub moov_offset: u64,
}

/// A built recording: its bytes as segments over a length, which may include a hole.
#[derive(Debug, Clone)]
pub struct Fixture {
    /// What was built.
    pub spec: Spec,
    /// What a reader is expected to find.
    pub expected: Expected,
    length: u64,
    segments: Vec<(u64, Vec<u8>)>,
}

impl Fixture {
    /// The total length in bytes.
    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    /// The whole recording in memory, or nothing when it is laid out sparsely.
    #[must_use]
    pub fn bytes(&self) -> Option<Vec<u8>> {
        if self.spec.layout.sparse {
            return None;
        }
        let mut out = Vec::with_capacity(usize::try_from(self.length).ok()?);
        for (offset, bytes) in &self.segments {
            if *offset != out.len() as u64 {
                return None;
            }
            out.extend_from_slice(bytes);
        }
        Some(out)
    }

    /// Writes the recording to a file. A hole is left unwritten, so a sparse variant occupies little disk.
    ///
    /// # Errors
    ///
    /// Returns the operating system's error.
    pub fn write_to(&self, path: &Path) -> std::io::Result<()> {
        let mut file = std::fs::File::create(path)?;
        for (offset, bytes) in &self.segments {
            file.seek(SeekFrom::Start(*offset))?;
            file.write_all(bytes)?;
        }
        file.set_len(self.length)?;
        file.flush()
    }

    /// Reads a range across the segments, with zeros in a hole.
    fn read_range_into(&self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        let length = into.len() as u64;
        let end = offset.checked_add(length).ok_or(SourceError::OutOfBounds {
            offset,
            length,
            available: self.length,
        })?;
        if end > self.length {
            return Err(SourceError::OutOfBounds {
                offset,
                length,
                available: self.length,
            });
        }

        into.fill(0);

        for (segment_offset, bytes) in &self.segments {
            let segment_end = segment_offset.saturating_add(bytes.len() as u64);
            let start = offset.max(*segment_offset);
            let stop = end.min(segment_end);
            if start >= stop {
                continue;
            }
            let from = usize::try_from(start.saturating_sub(*segment_offset)).unwrap_or(0);
            let to = usize::try_from(stop.saturating_sub(*segment_offset)).unwrap_or(0);
            let at = usize::try_from(start.saturating_sub(offset)).unwrap_or(0);
            if let (Some(source), Some(target)) = (
                bytes.get(from..to),
                into.get_mut(at..at.saturating_add(to.saturating_sub(from))),
            ) {
                target.copy_from_slice(source);
            }
        }

        Ok(())
    }
}

impl Source for Fixture {
    fn length(&mut self) -> Result<u64, SourceError> {
        Ok(self.length)
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        self.read_range_into(offset, into)
    }
}

/// Why a recording could not be built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BuildError {
    /// A committed stream could not be read.
    Frames(FrameError),
    /// A unit is too long for the requested length prefix.
    UnitTooLong {
        /// The unit's length.
        length: usize,
        /// The prefix width.
        nal_length_size: u8,
    },
    /// The prefix width is not one the container allows.
    BadLengthSize(u8),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Frames(cause) => {
                write!(formatter, "the committed frames could not be read: {cause}")
            },
            Self::UnitTooLong {
                length,
                nal_length_size,
            } => write!(
                formatter,
                "a unit of {length} bytes does not fit a {nal_length_size}-byte length prefix"
            ),
            Self::BadLengthSize(size) => {
                write!(formatter, "a length prefix of {size} bytes is not allowed")
            },
        }
    }
}

impl std::error::Error for BuildError {}

impl From<FrameError> for BuildError {
    fn from(cause: FrameError) -> Self {
        Self::Frames(cause)
    }
}

/// One track as it is about to be written.
struct Plan {
    role: Role,
    track_id: u32,
    timescale: u32,
    sample_duration: u32,
    samples: Vec<Vec<u8>>,
    sync: Option<Vec<u32>>,
    samples_per_chunk: u32,
    entry: Vec<u8>,
    configuration_payload: Vec<u8>,
    sequence_parameter_sets: Vec<Vec<u8>>,
    picture_parameter_sets: Vec<Vec<u8>>,
    width: u16,
    height: u16,
}

/// The pieces of a recording before they are placed.
struct Pieces {
    ftyp: Vec<u8>,
    top_level: Option<Vec<u8>>,
    mdat_payload: Vec<u8>,
    chunk_offsets: Vec<Vec<u64>>,
    moov_size: u64,
}

/// Builds a recording from a specification.
///
/// # Errors
///
/// Returns an error when the committed frames cannot be read or do not fit the requested layout.
pub fn build(spec: &Spec) -> Result<Fixture, BuildError> {
    if ![1, 2, 4].contains(&spec.nal_length_size) {
        return Err(BuildError::BadLengthSize(spec.nal_length_size));
    }

    let plans = plans_for(spec)?;
    let (mdat_payload, chunk_offsets) = interleave(&plans);

    let ftyp = boxed(
        *b"ftyp",
        &Fields::new()
            .bytes(b"mp42")
            .u32(0)
            .bytes(b"mp42")
            .bytes(b"mp41")
            .finish(),
    );
    let top_level = spec
        .vendor
        .top_level_box
        .then(|| boxed(TOP_LEVEL_BOX, &TOP_LEVEL_PAYLOAD));

    // The movie box's size does not depend on the offsets it stores, only on their width, so it is built
    // once with placeholder offsets to learn where the media will start, then again for real.
    let placeholders: Vec<Vec<u64>> = chunk_offsets
        .iter()
        .map(|offsets| vec![0; offsets.len()])
        .collect();
    let moov_size = build_moov(spec, &plans, &placeholders, 0).len() as u64;

    let pieces = Pieces {
        ftyp,
        top_level,
        mdat_payload,
        chunk_offsets,
        moov_size,
    };

    let (segments, length, mdat_offset, moov_offset) = if spec.layout.moov_first {
        place_moov_first(spec, &plans, pieces)
    } else {
        place_moov_last(spec, &plans, pieces)
    };

    let mdat_size = plans_mdat_size(&plans);

    let expected = Expected {
        tracks: plans.iter().map(expected_track).collect(),
        vendor_box: spec.vendor.udta_box.then(vendor_box),
        top_level_box: spec
            .vendor
            .top_level_box
            .then(|| boxed(TOP_LEVEL_BOX, &TOP_LEVEL_PAYLOAD)),
        mdat_offset,
        mdat_size,
        moov_offset,
    };

    Ok(Fixture {
        spec: spec.clone(),
        expected,
        length,
        segments,
    })
}

fn plans_for(spec: &Spec) -> Result<Vec<Plan>, BuildError> {
    let front = frames::video(frames::FRONT)?;
    let mut plans = vec![video_plan(spec, Role::Front, 1, &front)?];
    if spec.tracks.rear {
        let rear = frames::video(frames::REAR)?;
        plans.push(video_plan(spec, Role::Rear, 2, &rear)?);
    }
    if spec.tracks.audio {
        let audio = frames::audio(frames::AUDIO)?;
        let track_id = u32::try_from(plans.len()).unwrap_or(0).saturating_add(1);
        plans.push(audio_plan(spec, track_id, &audio));
    }
    Ok(plans)
}

fn plans_mdat_size(plans: &[Plan]) -> u64 {
    plans
        .iter()
        .flat_map(|plan| plan.samples.iter())
        .fold(8u64, |total, sample| total.saturating_add(sample.len() as u64))
}

fn expected_track(plan: &Plan) -> ExpectedTrack {
    ExpectedTrack {
        role: plan.role,
        track_id: plan.track_id,
        samples: plan.samples.clone(),
        sync_samples: plan
            .sync
            .clone()
            .unwrap_or_else(|| (1..=u32::try_from(plan.samples.len()).unwrap_or(0)).collect()),
        configuration_payload: plan.configuration_payload.clone(),
        sequence_parameter_sets: plan.sequence_parameter_sets.clone(),
        picture_parameter_sets: plan.picture_parameter_sets.clone(),
    }
}

type Placement = (Vec<(u64, Vec<u8>)>, u64, u64, u64);

/// Lays out `ftyp`, `moov`, the top-level vendor box, an optional hole, then `mdat`.
fn place_moov_first(spec: &Spec, plans: &[Plan], pieces: Pieces) -> Placement {
    let mut segments = Vec::new();
    let moov_offset = pieces.ftyp.len() as u64;
    let mut cursor = moov_offset.saturating_add(pieces.moov_size);
    if let Some(top) = &pieces.top_level {
        cursor = cursor.saturating_add(top.len() as u64);
    }

    let hole = spec.layout.sparse.then(|| {
        let free_offset = cursor;
        cursor = cursor.saturating_add(SPARSE_GAP);
        (free_offset, large_header(*b"free", SPARSE_GAP))
    });

    let mdat_offset = cursor;
    let mut head = pieces.ftyp;
    head.extend_from_slice(&build_moov(
        spec,
        plans,
        &pieces.chunk_offsets,
        mdat_offset.saturating_add(8),
    ));
    if let Some(top) = &pieces.top_level {
        head.extend_from_slice(top);
    }
    segments.push((0, head));
    if let Some(hole) = hole {
        segments.push(hole);
    }

    let mdat_size = pieces.mdat_payload.len() as u64 + 8;
    let mut mdat = header(*b"mdat", u32::try_from(mdat_size).unwrap_or(u32::MAX));
    mdat.extend_from_slice(&pieces.mdat_payload);
    segments.push((mdat_offset, mdat));

    (
        segments,
        mdat_offset.saturating_add(mdat_size),
        mdat_offset,
        moov_offset,
    )
}

/// Lays out `ftyp`, an optional hole, `mdat`, then `moov` and the top-level vendor box.
fn place_moov_last(spec: &Spec, plans: &[Plan], pieces: Pieces) -> Placement {
    let mut segments = Vec::new();
    let mut cursor = pieces.ftyp.len() as u64;
    segments.push((0, pieces.ftyp));

    if spec.layout.sparse {
        segments.push((cursor, large_header(*b"free", SPARSE_GAP)));
        cursor = cursor.saturating_add(SPARSE_GAP);
    }

    let mdat_offset = cursor;
    let mdat_size = pieces.mdat_payload.len() as u64 + 8;
    let mut mdat = header(*b"mdat", u32::try_from(mdat_size).unwrap_or(u32::MAX));
    mdat.extend_from_slice(&pieces.mdat_payload);
    segments.push((mdat_offset, mdat));
    cursor = cursor.saturating_add(mdat_size);

    let moov_offset = cursor;
    let mut tail = build_moov(spec, plans, &pieces.chunk_offsets, mdat_offset.saturating_add(8));
    if let Some(top) = &pieces.top_level {
        tail.extend_from_slice(top);
    }
    cursor = cursor.saturating_add(tail.len() as u64);
    segments.push((moov_offset, tail));

    (segments, cursor, mdat_offset, moov_offset)
}

/// Writes every recording of the corpus into a directory, named after its specification.
///
/// # Errors
///
/// Returns a build error or the operating system's error.
pub fn write_corpus(directory: &Path) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    let mut written = Vec::new();
    for spec in corpus() {
        let fixture = build(&spec)?;
        let path = directory.join(format!("{}.mp4", spec.name));
        fixture.write_to(&path)?;
        written.push(path);
    }
    Ok(written)
}

fn prefixed(units: &[Vec<u8>], nal_length_size: u8) -> Result<Vec<u8>, BuildError> {
    let mut out = Vec::new();
    for unit in units {
        let length = unit.len();
        let fits = match nal_length_size {
            1 => u8::try_from(length).is_ok(),
            2 => u16::try_from(length).is_ok(),
            4 => u32::try_from(length).is_ok(),
            _ => false,
        };
        if !fits {
            return Err(BuildError::UnitTooLong {
                length,
                nal_length_size,
            });
        }
        let encoded = (length as u64).to_be_bytes();
        out.extend_from_slice(
            encoded
                .get(8usize.saturating_sub(usize::from(nal_length_size))..)
                .unwrap_or(&[]),
        );
        out.extend_from_slice(unit);
    }
    Ok(out)
}

fn avc_configuration(spec: &Spec, stream: &VideoStream) -> Vec<u8> {
    let sps = stream
        .sequence_parameter_sets
        .first()
        .cloned()
        .unwrap_or_default();
    let mut configuration = Fields::new()
        .u8(1)
        .u8(sps.get(1).copied().unwrap_or(0))
        .u8(sps.get(2).copied().unwrap_or(0))
        .u8(sps.get(3).copied().unwrap_or(0))
        .u8(0xfc | spec.nal_length_size.saturating_sub(1))
        .u8(0xe0 | u8::try_from(stream.sequence_parameter_sets.len()).unwrap_or(1));
    for set in &stream.sequence_parameter_sets {
        configuration = configuration
            .u16(u16::try_from(set.len()).unwrap_or(u16::MAX))
            .bytes(set);
    }
    configuration = configuration.u8(u8::try_from(stream.picture_parameter_sets.len()).unwrap_or(1));
    for set in &stream.picture_parameter_sets {
        configuration = configuration
            .u16(u16::try_from(set.len()).unwrap_or(u16::MAX))
            .bytes(set);
    }
    configuration.finish()
}

fn video_plan(spec: &Spec, role: Role, track_id: u32, stream: &VideoStream) -> Result<Plan, BuildError> {
    let samples = stream
        .access_units
        .iter()
        .map(|unit| prefixed(&unit.units, spec.nal_length_size))
        .collect::<Result<Vec<_>, _>>()?;
    let sync: Vec<u32> = stream
        .access_units
        .iter()
        .enumerate()
        .filter(|(_, unit)| unit.sync)
        .map(|(index, _)| u32::try_from(index).unwrap_or(0).saturating_add(1))
        .collect();

    let configuration_payload = avc_configuration(spec, stream);

    let mut compressor = [0u8; 32];
    let name = spec.compressor.as_bytes();
    let name_length = name.len().min(31);
    compressor[0] = u8::try_from(name_length).unwrap_or(31);
    for (slot, byte) in compressor.iter_mut().skip(1).zip(name.iter().take(name_length)) {
        *slot = *byte;
    }

    let mut entry_payload = Fields::new()
        .zeros(6)
        .u16(1)
        .zeros(16)
        .u16(FRAME_SIZE)
        .u16(FRAME_SIZE)
        .u32(0x0048_0000)
        .u32(0x0048_0000)
        .u32(0)
        .u16(1)
        .bytes(&compressor)
        .u16(0x0018)
        .i16(-1)
        .finish();
    entry_payload.extend_from_slice(&boxed(*b"avcC", &configuration_payload));
    if spec.quirks.unknown_child_in_entry && role == Role::Front {
        entry_payload.extend_from_slice(&boxed(ENTRY_CHILD_BOX, b"opaque entry child"));
    }

    Ok(Plan {
        role,
        track_id,
        timescale: VIDEO_TIMESCALE,
        sample_duration: VIDEO_SAMPLE_DURATION,
        samples,
        sync: Some(sync),
        samples_per_chunk: spec.chunking.video.max(1),
        entry: boxed(*b"avc1", &entry_payload),
        configuration_payload,
        sequence_parameter_sets: stream.sequence_parameter_sets.clone(),
        picture_parameter_sets: stream.picture_parameter_sets.clone(),
        width: FRAME_SIZE,
        height: FRAME_SIZE,
    })
}

fn audio_plan(spec: &Spec, track_id: u32, stream: &AudioStream) -> Plan {
    let decoder_specific = descriptor(5, &stream.specific_configuration());
    let mut decoder_config = Fields::new().u8(0x40).u8(0x15).zeros(3).u32(0).u32(0).finish();
    decoder_config.extend_from_slice(&decoder_specific);
    let mut es = Fields::new().u16(0).u8(0).finish();
    es.extend_from_slice(&descriptor(4, &decoder_config));
    es.extend_from_slice(&descriptor(6, &[0x02]));
    let esds_payload = descriptor(3, &es);

    let mut entry_payload = Fields::new()
        .zeros(6)
        .u16(1)
        .u16(0)
        .u16(0)
        .u32(0)
        .u16(u16::from(stream.channel_configuration))
        .u16(16)
        .u16(0)
        .u16(0)
        .u32(stream.sample_rate() << 16)
        .finish();
    entry_payload.extend_from_slice(&full_box(*b"esds", 0, 0, &esds_payload));

    Plan {
        role: Role::Audio,
        track_id,
        timescale: stream.sample_rate(),
        sample_duration: AUDIO_SAMPLE_DURATION,
        samples: stream.frames.clone(),
        sync: None,
        samples_per_chunk: spec.chunking.audio.max(1),
        entry: boxed(*b"mp4a", &entry_payload),
        // The elementary stream descriptor box is a full box: its payload opens with version and flags.
        configuration_payload: [&[0, 0, 0, 0][..], &esds_payload].concat(),
        sequence_parameter_sets: Vec::new(),
        picture_parameter_sets: Vec::new(),
        width: 0,
        height: 0,
    }
}

/// Lays the samples of every track into one payload, chunk by chunk, round robin across tracks, and
/// returns the offset of each chunk within the payload, per track.
fn interleave(plans: &[Plan]) -> (Vec<u8>, Vec<Vec<u64>>) {
    let mut payload = Vec::new();
    let mut offsets: Vec<Vec<u64>> = plans.iter().map(|_| Vec::new()).collect();
    let mut next_sample: Vec<usize> = plans.iter().map(|_| 0).collect();

    loop {
        let mut progressed = false;
        for (index, plan) in plans.iter().enumerate() {
            let start = next_sample.get(index).copied().unwrap_or(plan.samples.len());
            if start >= plan.samples.len() {
                continue;
            }
            let per_chunk = usize::try_from(plan.samples_per_chunk).unwrap_or(1);
            let end = start.saturating_add(per_chunk).min(plan.samples.len());
            if let Some(list) = offsets.get_mut(index) {
                list.push(payload.len() as u64);
            }
            for sample in plan.samples.get(start..end).unwrap_or(&[]) {
                payload.extend_from_slice(sample);
            }
            if let Some(slot) = next_sample.get_mut(index) {
                *slot = end;
            }
            progressed = true;
        }
        if !progressed {
            break;
        }
    }

    (payload, offsets)
}

fn scaled(ticks: u64, from: u32, to: u32) -> u64 {
    let product = u128::from(ticks).saturating_mul(u128::from(to));
    let scaled = product.checked_div(u128::from(from)).unwrap_or(0);
    u64::try_from(scaled).unwrap_or(u64::MAX)
}

fn build_moov(spec: &Spec, plans: &[Plan], chunk_offsets: &[Vec<u64>], media_base: u64) -> Vec<u8> {
    let movie_duration = plans
        .iter()
        .map(|plan| {
            scaled(
                u64::from(plan.sample_duration).saturating_mul(plan.samples.len() as u64),
                plan.timescale,
                MOVIE_TIMESCALE,
            )
        })
        .max()
        .unwrap_or(0);

    let mvhd = full_box(
        *b"mvhd",
        1,
        0,
        &Fields::new()
            .u64(CREATION_TIME)
            .u64(CREATION_TIME)
            .u32(MOVIE_TIMESCALE)
            .u64(movie_duration)
            .u32(0x0001_0000)
            .u16(0x0100)
            .zeros(10)
            .unity_matrix()
            .zeros(24)
            .u32(u32::try_from(plans.len()).unwrap_or(0).saturating_add(1))
            .finish(),
    );

    let mut children = vec![mvhd];

    for (index, plan) in plans.iter().enumerate() {
        let offsets = chunk_offsets.get(index).map_or(&[][..], Vec::as_slice);
        children.push(build_trak(spec, plan, offsets, media_base));
    }

    if spec.vendor.udta_box {
        children.push(container(*b"udta", &[vendor_box()]));
    }

    container(*b"moov", &children)
}

/// The synthetic vendor box: a header, a marker string, then deterministic bytes that no parser reads.
fn vendor_box() -> Vec<u8> {
    let mut payload = Vec::with_capacity(VENDOR_BOX_SIZE - 8);
    payload.extend_from_slice(b"$SYNTHETIC VENDOR RECORD V0.01 CH:2\n");
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    while payload.len() < VENDOR_BOX_SIZE - 8 {
        // A xorshift sequence: deterministic, incompressible enough, and nothing anyone would decode.
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        payload.push(u8::try_from(state & 0xff).unwrap_or(0));
    }
    boxed(VENDOR_BOX, &payload)
}

fn track_header(plan: &Plan, track_duration: u64) -> Vec<u8> {
    full_box(
        *b"tkhd",
        1,
        0x0000_0001,
        &Fields::new()
            .u64(CREATION_TIME)
            .u64(CREATION_TIME)
            .u32(plan.track_id)
            .u32(0)
            .u64(track_duration)
            .zeros(8)
            .i16(0)
            .i16(0)
            .u16(if plan.role == Role::Audio { 0x0100 } else { 0 })
            .u16(0)
            .unity_matrix()
            .u32(u32::from(plan.width) << 16)
            .u32(u32::from(plan.height) << 16)
            .finish(),
    )
}

fn media_information(plan: &Plan, chunk_offsets: &[u64], wide: bool, media_base: u64) -> Vec<u8> {
    let media_header = match plan.role {
        Role::Audio => full_box(*b"smhd", 0, 0, &Fields::new().u16(0).u16(0).finish()),
        Role::Front | Role::Rear => full_box(
            *b"vmhd",
            0,
            1,
            &Fields::new().u16(0).u16(0).u16(0).u16(0).finish(),
        ),
    };
    let dinf = container(
        *b"dinf",
        &[full_box(*b"dref", 0, 0, &{
            let mut payload = Fields::new().u32(1).finish();
            payload.extend_from_slice(&full_box(*b"url ", 0, 1, &[]));
            payload
        })],
    );
    let stbl = sample_tables(plan, chunk_offsets, wide, media_base);
    container(*b"minf", &[media_header, dinf, stbl])
}

fn sample_tables(plan: &Plan, chunk_offsets: &[u64], wide: bool, media_base: u64) -> Vec<u8> {
    let sample_count = u32::try_from(plan.samples.len()).unwrap_or(0);

    let description = full_box(*b"stsd", 0, 0, &{
        let mut payload = Fields::new().u32(1).finish();
        payload.extend_from_slice(&plan.entry);
        payload
    });
    let timing = full_box(
        *b"stts",
        0,
        0,
        &Fields::new()
            .u32(1)
            .u32(sample_count)
            .u32(plan.sample_duration)
            .finish(),
    );
    let sync_table = plan.sync.as_ref().map(|sync| {
        let mut payload = Fields::new().u32(u32::try_from(sync.len()).unwrap_or(0)).finish();
        for number in sync {
            payload.extend_from_slice(&number.to_be_bytes());
        }
        full_box(*b"stss", 0, 0, &payload)
    });

    // One run covers every chunk when the chunks are all full; a shorter last chunk needs its own run.
    let per_chunk = plan.samples_per_chunk;
    let full_chunks = sample_count.checked_div(per_chunk).unwrap_or(0);
    let remainder = sample_count.checked_rem(per_chunk).unwrap_or(0);
    let mut runs: Vec<(u32, u32)> = Vec::new();
    if full_chunks > 0 {
        runs.push((1, per_chunk));
    }
    if remainder > 0 {
        runs.push((full_chunks.saturating_add(1), remainder));
    }
    let chunk_runs = full_box(*b"stsc", 0, 0, &{
        let mut payload = Fields::new().u32(u32::try_from(runs.len()).unwrap_or(0)).finish();
        for (first, count) in &runs {
            payload.extend_from_slice(&first.to_be_bytes());
            payload.extend_from_slice(&count.to_be_bytes());
            payload.extend_from_slice(&1u32.to_be_bytes());
        }
        payload
    });

    let sizes = full_box(*b"stsz", 0, 0, &{
        let mut payload = Fields::new().u32(0).u32(sample_count).finish();
        for sample in &plan.samples {
            payload.extend_from_slice(&u32::try_from(sample.len()).unwrap_or(u32::MAX).to_be_bytes());
        }
        payload
    });

    let chunk_count = u32::try_from(chunk_offsets.len()).unwrap_or(0);
    let offsets = if wide {
        full_box(*b"co64", 0, 0, &{
            let mut payload = Fields::new().u32(chunk_count).finish();
            for offset in chunk_offsets {
                payload.extend_from_slice(&media_base.saturating_add(*offset).to_be_bytes());
            }
            payload
        })
    } else {
        full_box(*b"stco", 0, 0, &{
            let mut payload = Fields::new().u32(chunk_count).finish();
            for offset in chunk_offsets {
                let absolute = u32::try_from(media_base.saturating_add(*offset)).unwrap_or(u32::MAX);
                payload.extend_from_slice(&absolute.to_be_bytes());
            }
            payload
        })
    };

    let mut children = vec![description, timing];
    if let Some(sync_table) = sync_table {
        children.push(sync_table);
    }
    children.extend([chunk_runs, sizes, offsets]);
    container(*b"stbl", &children)
}

fn build_trak(spec: &Spec, plan: &Plan, chunk_offsets: &[u64], media_base: u64) -> Vec<u8> {
    let sample_count = u64::from(u32::try_from(plan.samples.len()).unwrap_or(0));
    let media_duration = u64::from(plan.sample_duration).saturating_mul(sample_count);
    let track_duration = scaled(media_duration, plan.timescale, MOVIE_TIMESCALE);

    let tkhd = track_header(plan, track_duration);

    let edts = spec.quirks.edit_list.then(|| {
        container(
            *b"edts",
            &[full_box(
                *b"elst",
                1,
                0,
                &Fields::new()
                    .u32(1)
                    .u64(track_duration)
                    .u64(u64::from(plan.sample_duration))
                    .i32(0x0001_0000)
                    .finish(),
            )],
        )
    });

    let mdhd = full_box(
        *b"mdhd",
        1,
        0,
        &Fields::new()
            .u64(CREATION_TIME)
            .u64(CREATION_TIME)
            .u32(plan.timescale)
            .u64(media_duration)
            .u16(0x55c4)
            .u16(0)
            .finish(),
    );

    let (handler, name): ([u8; 4], &[u8]) = match plan.role {
        Role::Audio => (*b"soun", b"SoundHandler\0"),
        Role::Front | Role::Rear => (*b"vide", b"VideoHandler\0"),
    };
    let hdlr = full_box(
        *b"hdlr",
        0,
        0,
        &Fields::new()
            .u32(0)
            .bytes(&handler)
            .zeros(12)
            .bytes(name)
            .finish(),
    );

    let minf = media_information(plan, chunk_offsets, spec.wide_offsets(), media_base);
    let mdia = container(*b"mdia", &[mdhd, hdlr, minf]);

    let mut children = vec![tkhd];
    if let Some(edts) = edts {
        children.push(edts);
    }
    children.push(mdia);
    container(*b"trak", &children)
}

#[cfg(test)]
mod tests {
    use super::{Role, SPARSE_GAP, Spec, VENDOR_BOX_SIZE, build, corpus};
    use adiungere_isobmff::Source;

    #[test]
    fn the_reference_like_recording_lays_the_movie_box_before_the_media() {
        // Act
        let fixture = build(&Spec::reference_like()).unwrap();
        let bytes = fixture.bytes().unwrap();

        // Assert
        assert_eq!(bytes.get(4..8), Some(&b"ftyp"[..]));
        let moov_at = bytes.windows(4).position(|window| window == b"moov").unwrap();
        let mdat_at = bytes.windows(4).position(|window| window == b"mdat").unwrap();
        assert!(moov_at < mdat_at);
        assert_eq!(fixture.expected.tracks.len(), 3);
        assert_eq!(
            fixture.expected.vendor_box.as_ref().map(Vec::len),
            Some(VENDOR_BOX_SIZE)
        );
    }

    #[test]
    fn every_corpus_entry_builds_and_has_a_distinct_name() {
        // Act
        let specs = corpus();
        let built: Vec<_> = specs.iter().map(|spec| build(spec).unwrap()).collect();

        // Assert
        let mut names: Vec<&str> = specs.iter().map(|spec| spec.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), specs.len());
        assert!(built.iter().all(|fixture| fixture.length() > 0));
    }

    #[test]
    fn the_sparse_recording_reads_zeros_in_its_hole_and_media_beyond_four_gigabytes() {
        // Arrange
        let spec = corpus().into_iter().find(|spec| spec.layout.sparse).unwrap();
        let mut fixture = build(&spec).unwrap();

        // Act
        let in_hole = fixture.read_range(1 << 31, 16).unwrap();
        let mdat = fixture.read_range(fixture.expected.mdat_offset, 8).unwrap();

        // Assert
        assert_eq!(in_hole, vec![0; 16]);
        assert_eq!(mdat.get(4..8), Some(&b"mdat"[..]));
        assert!(fixture.expected.mdat_offset > SPARSE_GAP);
        assert!(fixture.bytes().is_none());
    }

    #[test]
    fn the_rewritten_recording_has_one_video_track_and_no_vendor_box() {
        // Arrange
        let spec = corpus()
            .into_iter()
            .find(|spec| spec.name == "rewritten")
            .unwrap();

        // Act
        let fixture = build(&spec).unwrap();

        // Assert
        let roles: Vec<Role> = fixture.expected.tracks.iter().map(|track| track.role).collect();
        assert_eq!(roles, vec![Role::Front, Role::Audio]);
        assert!(fixture.expected.vendor_box.is_none());
        assert!(fixture.expected.top_level_box.is_none());
    }

    #[test]
    fn a_length_prefix_the_container_does_not_allow_is_refused() {
        // Arrange
        let spec = Spec {
            nal_length_size: 3,
            ..Spec::reference_like()
        };

        // Act
        let outcome = build(&spec);

        // Assert
        assert!(outcome.is_err());
    }

    #[test]
    fn the_in_memory_bytes_and_the_segment_reads_agree() {
        // Arrange
        let mut fixture = build(&Spec::reference_like()).unwrap();
        let bytes = fixture.bytes().unwrap();

        // Act
        let read = fixture.read_range(0, bytes.len()).unwrap();

        // Assert
        assert_eq!(read, bytes);
    }
}
