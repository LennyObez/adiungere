//! Typed views: the few boxes whose values are needed to locate samples and describe tracks.
//!
//! A view is read from a range and never replaces it. Nothing here is written back, and nothing here is
//! resynthesised: a decoder configuration is exposed as the bytes it was stored as, with a parsed reading
//! beside it for the values a fingerprint needs.

use serde::Serialize;

use crate::error::Error;
use crate::fourcc::FourCc;
use crate::parse::Container;
use crate::samples::SampleTable;
use crate::tree::BoxRange;

/// A bounds-checked reader over a box payload that names the field it could not read.
struct Fields<'a> {
    bytes: &'a [u8],
    at: usize,
    kind: FourCc,
    offset: u64,
}

impl<'a> Fields<'a> {
    const fn new(bytes: &'a [u8], kind: FourCc, offset: u64) -> Self {
        Self {
            bytes,
            at: 0,
            kind,
            offset,
        }
    }

    fn take(&mut self, count: usize, field: &'static str) -> Result<&'a [u8], Error> {
        let end = self.at.checked_add(count).ok_or(Error::Truncated {
            kind: self.kind,
            offset: self.offset,
            field,
        })?;
        let slice = self.bytes.get(self.at..end).ok_or(Error::Truncated {
            kind: self.kind,
            offset: self.offset,
            field,
        })?;
        self.at = end;
        Ok(slice)
    }

    fn u8(&mut self, field: &'static str) -> Result<u8, Error> {
        Ok(self.take(1, field)?.first().copied().unwrap_or(0))
    }

    fn u16(&mut self, field: &'static str) -> Result<u16, Error> {
        let slice = self.take(2, field)?;
        Ok(u16::from_be_bytes(slice.try_into().unwrap_or([0; 2])))
    }

    fn u32(&mut self, field: &'static str) -> Result<u32, Error> {
        let slice = self.take(4, field)?;
        Ok(u32::from_be_bytes(slice.try_into().unwrap_or([0; 4])))
    }

    fn u64(&mut self, field: &'static str) -> Result<u64, Error> {
        let slice = self.take(8, field)?;
        Ok(u64::from_be_bytes(slice.try_into().unwrap_or([0; 8])))
    }

    fn i16(&mut self, field: &'static str) -> Result<i16, Error> {
        let slice = self.take(2, field)?;
        Ok(i16::from_be_bytes(slice.try_into().unwrap_or([0; 2])))
    }

    fn i32(&mut self, field: &'static str) -> Result<i32, Error> {
        let slice = self.take(4, field)?;
        Ok(i32::from_be_bytes(slice.try_into().unwrap_or([0; 4])))
    }

    fn i64(&mut self, field: &'static str) -> Result<i64, Error> {
        let slice = self.take(8, field)?;
        Ok(i64::from_be_bytes(slice.try_into().unwrap_or([0; 8])))
    }

    /// A 32-bit or 64-bit field, as the box version decides.
    fn versioned(&mut self, version: u8, field: &'static str) -> Result<u64, Error> {
        if version == 0 {
            self.u32(field).map(u64::from)
        } else {
            self.u64(field)
        }
    }

    fn skip(&mut self, count: usize, field: &'static str) -> Result<(), Error> {
        self.take(count, field).map(|_| ())
    }

    fn version_and_flags(&mut self) -> Result<(u8, u32), Error> {
        let version = self.u8("version")?;
        let flags = self.take(3, "flags")?;
        // Three bytes, right-aligned into four, are the number itself.
        let mut padded = [0u8; 4];
        if let Some(tail) = padded.get_mut(1..) {
            tail.copy_from_slice(flags);
        }
        Ok((version, u32::from_be_bytes(padded)))
    }

    fn rest(&self) -> &'a [u8] {
        self.bytes.get(self.at..).unwrap_or(&[])
    }
}

fn known_version(version: u8, allowed: &[u8], kind: FourCc, offset: u64) -> Result<(), Error> {
    if allowed.contains(&version) {
        Ok(())
    } else {
        Err(Error::UnknownVersion {
            kind,
            offset,
            version,
        })
    }
}

/// The file type box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ftyp {
    /// The brand the writer claims.
    pub major_brand: FourCc,
    /// The writer's version of that brand.
    pub minor_version: u32,
    /// The brands the file also conforms to.
    pub compatible_brands: Vec<FourCc>,
}

impl Ftyp {
    /// Reads the box from its payload, or nothing when the payload is too short to hold a brand.
    #[must_use]
    pub fn read(payload: &[u8]) -> Option<Self> {
        let mut fields = Fields::new(payload, FourCc(*b"ftyp"), 0);
        let major_brand = FourCc(fields.take(4, "major brand").ok()?.try_into().ok()?);
        let minor_version = fields.u32("minor version").ok()?;
        let compatible_brands = fields
            .rest()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| FourCc(*chunk))
            .collect();

        Some(Self {
            major_brand,
            minor_version,
            compatible_brands,
        })
    }
}

/// The movie header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mvhd {
    /// The box version, which decides the width of the time fields.
    pub version: u8,
    /// Seconds since 1904-01-01T00:00:00Z, as written by the recorder. Not to be trusted as UTC.
    pub creation_time: u64,
    /// Seconds since 1904-01-01T00:00:00Z, as written by the recorder.
    pub modification_time: u64,
    /// Ticks per second for the durations in this header.
    pub timescale: u32,
    /// The presentation duration in ticks.
    pub duration: u64,
    /// The identifier the next added track would receive.
    pub next_track_id: u32,
}

impl Mvhd {
    /// Reads the header from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when a field is missing or the version is unknown.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"mvhd");
        let mut fields = Fields::new(payload, kind, offset);
        let (version, _) = fields.version_and_flags()?;
        known_version(version, &[0, 1], kind, offset)?;
        let creation_time = fields.versioned(version, "creation time")?;
        let modification_time = fields.versioned(version, "modification time")?;
        let timescale = fields.u32("timescale")?;
        let duration = fields.versioned(version, "duration")?;
        fields.skip(4 + 2 + 10 + 36 + 24, "rate, volume, matrix and predefined fields")?;
        let next_track_id = fields.u32("next track identifier")?;

        Ok(Self {
            version,
            creation_time,
            modification_time,
            timescale,
            duration,
            next_track_id,
        })
    }
}

/// The track header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Tkhd {
    /// The box version.
    pub version: u8,
    /// The flags: enabled, in movie, in preview.
    pub flags: u32,
    /// Seconds since 1904-01-01T00:00:00Z, as written by the recorder.
    pub creation_time: u64,
    /// Seconds since 1904-01-01T00:00:00Z, as written by the recorder.
    pub modification_time: u64,
    /// The track identifier.
    pub track_id: u32,
    /// The duration in movie timescale ticks.
    pub duration: u64,
    /// The layer, front to back.
    pub layer: i16,
    /// The alternate group, zero when the track belongs to none.
    pub alternate_group: i16,
    /// The volume as 8.8 fixed point.
    pub volume: u16,
    /// The presentation width as 16.16 fixed point.
    pub width: u32,
    /// The presentation height as 16.16 fixed point.
    pub height: u32,
}

impl Tkhd {
    /// The flag a player reads to decide whether the track exists for playback.
    pub const ENABLED: u32 = 0x1;
    /// The flag a player reads to decide whether the track is part of the presentation.
    pub const IN_MOVIE: u32 = 0x2;
    /// The flag a player reads to decide whether the track is part of the preview.
    pub const IN_PREVIEW: u32 = 0x4;

    /// Reads the header from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when a field is missing or the version is unknown.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"tkhd");
        let mut fields = Fields::new(payload, kind, offset);
        let (version, flags) = fields.version_and_flags()?;
        known_version(version, &[0, 1], kind, offset)?;
        let creation_time = fields.versioned(version, "creation time")?;
        let modification_time = fields.versioned(version, "modification time")?;
        let track_id = fields.u32("track identifier")?;
        fields.skip(4, "reserved")?;
        let duration = fields.versioned(version, "duration")?;
        fields.skip(8, "reserved")?;
        let layer = fields.i16("layer")?;
        let alternate_group = fields.i16("alternate group")?;
        let volume = fields.u16("volume")?;
        fields.skip(2 + 36, "reserved and matrix")?;
        let width = fields.u32("width")?;
        let height = fields.u32("height")?;

        Ok(Self {
            version,
            flags,
            creation_time,
            modification_time,
            track_id,
            duration,
            layer,
            alternate_group,
            volume,
            width,
            height,
        })
    }

    /// Whether the enabled flag is set.
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.flags & Self::ENABLED != 0
    }
}

/// The media header.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Mdhd {
    /// The box version.
    pub version: u8,
    /// Seconds since 1904-01-01T00:00:00Z, as written by the recorder.
    pub creation_time: u64,
    /// Seconds since 1904-01-01T00:00:00Z, as written by the recorder.
    pub modification_time: u64,
    /// Ticks per second for this track's samples.
    pub timescale: u32,
    /// The media duration in ticks.
    pub duration: u64,
    /// The language as three lower-case letters, decoded from the packed field.
    pub language: String,
}

impl Mdhd {
    /// Reads the header from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when a field is missing or the version is unknown.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"mdhd");
        let mut fields = Fields::new(payload, kind, offset);
        let (version, _) = fields.version_and_flags()?;
        known_version(version, &[0, 1], kind, offset)?;
        let creation_time = fields.versioned(version, "creation time")?;
        let modification_time = fields.versioned(version, "modification time")?;
        let timescale = fields.u32("timescale")?;
        let duration = fields.versioned(version, "duration")?;
        let packed = fields.u16("language")?;
        let language = [10, 5, 0]
            .iter()
            .map(|shift| char::from(u8::try_from(((packed >> shift) & 0x1f) + 0x60).unwrap_or(b'?')))
            .collect();

        Ok(Self {
            version,
            creation_time,
            modification_time,
            timescale,
            duration,
            language,
        })
    }
}

/// The handler reference, which says what kind of media a track carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Hdlr {
    /// The handler type: `vide`, `soun`, or something else.
    pub handler_type: FourCc,
    /// The human-readable name, which some recorders use for an encoder tag.
    pub name: String,
}

impl Hdlr {
    /// Reads the box from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when a field is missing.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"hdlr");
        let mut fields = Fields::new(payload, kind, offset);
        fields.version_and_flags()?;
        fields.skip(4, "predefined")?;
        let handler_type = FourCc(fields.take(4, "handler type")?.try_into().unwrap_or([0; 4]));
        fields.skip(12, "reserved")?;
        let raw = fields.rest();
        let name = String::from_utf8_lossy(raw).trim_end_matches('\0').to_owned();

        Ok(Self { handler_type, name })
    }
}

/// What a track carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    /// A video track.
    Video,
    /// An audio track.
    Audio,
    /// Something else, named by its handler type.
    Other(FourCc),
}

/// The visual fields of a sample entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VisualFields {
    /// The coded width in pixels.
    pub width: u16,
    /// The coded height in pixels.
    pub height: u16,
    /// How many frames one sample holds, normally one.
    pub frame_count: u16,
    /// The compressor name the writer stored, which is where an encoder tag usually lives.
    pub compressor_name: String,
    /// The bit depth field.
    pub depth: u16,
}

/// The audio fields of a sample entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AudioFields {
    /// The entry version, which decides the layout.
    pub version: u16,
    /// The channel count.
    pub channel_count: u16,
    /// The sample size in bits.
    pub sample_size: u16,
    /// The sample rate, integer part of the 16.16 field.
    pub sample_rate: u32,
}

/// One entry of a sample description: a range, its fixed fields read, and its children as ranges.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SampleEntry {
    /// The entry, header and children included.
    pub range: BoxRange,
    /// The coding, which is the entry's type.
    pub kind: FourCc,
    /// Which data reference the samples are found through.
    pub data_reference_index: u16,
    /// The visual fields, for a visual entry.
    pub visual: Option<VisualFields>,
    /// The audio fields, for an audio entry.
    pub audio: Option<AudioFields>,
    /// The decoder configuration child, as a range, when the entry has one this reader recognises.
    pub configuration: Option<BoxRange>,
}

impl SampleEntry {
    /// Reads an entry from its range and the bytes behind it.
    fn read(container: &Container, range: &BoxRange) -> Self {
        let kind = range.kind;
        let bytes = container.bytes_of(range).unwrap_or(&[]);
        let mut fields = Fields::new(bytes, kind, range.offset);
        let _ = fields.skip(usize::from(range.header), "header");
        let _ = fields.skip(6, "reserved");
        let data_reference_index = fields.u16("data reference index").unwrap_or(0);

        let visual = VISUAL_ENTRIES.contains(&&kind.bytes()).then(|| {
            let mut visual = Fields::new(bytes, kind, range.offset);
            let _ = visual.skip(usize::from(range.header) + 6 + 2 + 16, "predefined");
            let width = visual.u16("width").unwrap_or(0);
            let height = visual.u16("height").unwrap_or(0);
            let _ = visual.skip(4 + 4 + 4, "resolutions and reserved");
            let frame_count = visual.u16("frame count").unwrap_or(0);
            let name = visual.take(32, "compressor name").unwrap_or(&[]);
            // A counted string; some writers count a terminating zero into the length, and the zero is
            // not part of the name.
            let name_length = usize::from(name.first().copied().unwrap_or(0)).min(31);
            let compressor_name = String::from_utf8_lossy(name.get(1..=name_length).unwrap_or(&[]))
                .trim_end_matches('\0')
                .to_owned();
            let depth = visual.u16("depth").unwrap_or(0);
            VisualFields {
                width,
                height,
                frame_count,
                compressor_name,
                depth,
            }
        });

        let audio = AUDIO_ENTRIES.contains(&&kind.bytes()).then(|| {
            let mut audio = Fields::new(bytes, kind, range.offset);
            // Six reserved bytes and the data reference index; then the version; then the revision level
            // and the vendor, six bytes; then the channel count and the sample size; then the compression
            // identifier and the packet size, four bytes; then the sample rate.
            let _ = audio.skip(usize::from(range.header), "header");
            let _ = audio.skip(8, "reserved and data reference index");
            let version = audio.u16("version").unwrap_or(0);
            let _ = audio.skip(6, "revision and vendor");
            let channel_count = audio.u16("channel count").unwrap_or(0);
            let sample_size = audio.u16("sample size").unwrap_or(0);
            let _ = audio.skip(4, "compression identifier and packet size");
            let sample_rate = audio.u32("sample rate").unwrap_or(0) >> 16;
            AudioFields {
                version,
                channel_count,
                sample_size,
                sample_rate,
            }
        });

        let configuration = range
            .children
            .iter()
            .find(|child| CONFIGURATION_BOXES.contains(&&child.kind.bytes()))
            .cloned();

        Self {
            range: range.clone(),
            kind,
            data_reference_index,
            visual,
            audio,
            configuration,
        }
    }

    /// The parsed AVC decoder configuration, when the entry carries one.
    #[must_use]
    pub fn avc_configuration(&self, container: &Container) -> Option<AvcConfiguration> {
        let configuration = self.configuration.as_ref()?;
        if configuration.kind != b"avcC" {
            return None;
        }
        AvcConfiguration::read(container.payload_of(configuration)?)
    }

    /// The codec string in the form registered for the web, such as `avc1.640028`, when it can be derived.
    #[must_use]
    pub fn codec_string(&self, container: &Container) -> Option<String> {
        let avc = self.avc_configuration(container)?;
        Some(format!(
            "{}.{:02X}{:02X}{:02X}",
            self.kind, avc.profile, avc.compatibility, avc.level
        ))
    }
}

const VISUAL_ENTRIES: [&[u8; 4]; 6] = [b"avc1", b"avc3", b"hvc1", b"hev1", b"mp4v", b"encv"];
const AUDIO_ENTRIES: [&[u8; 4]; 4] = [b"mp4a", b"enca", b"ac-3", b"ec-3"];
const CONFIGURATION_BOXES: [&[u8; 4]; 5] = [b"avcC", b"hvcC", b"esds", b"dac3", b"dec3"];

/// The AVC decoder configuration record, read from the bytes it was stored as.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AvcConfiguration {
    /// The profile indication.
    pub profile: u8,
    /// The profile compatibility flags.
    pub compatibility: u8,
    /// The level indication.
    pub level: u8,
    /// How many bytes each unit's length prefix occupies in a sample: 1, 2 or 4.
    pub nal_length_size: u8,
    /// The sequence parameter sets, each as stored.
    pub sequence_parameter_sets: Vec<Vec<u8>>,
    /// The picture parameter sets, each as stored.
    pub picture_parameter_sets: Vec<Vec<u8>>,
}

impl AvcConfiguration {
    /// Reads the record from the payload of an `avcC` box, or nothing when it is malformed.
    #[must_use]
    pub fn read(payload: &[u8]) -> Option<Self> {
        let mut fields = Fields::new(payload, FourCc(*b"avcC"), 0);
        let version = fields.u8("configuration version").ok()?;
        if version != 1 {
            return None;
        }
        let profile = fields.u8("profile").ok()?;
        let compatibility = fields.u8("compatibility").ok()?;
        let level = fields.u8("level").ok()?;
        let nal_length_size = (fields.u8("length size").ok()? & 0x03) + 1;
        let sps_count = fields.u8("sequence parameter set count").ok()? & 0x1f;
        let sequence_parameter_sets = read_parameter_sets(&mut fields, sps_count)?;
        let pps_count = fields.u8("picture parameter set count").ok()?;
        let picture_parameter_sets = read_parameter_sets(&mut fields, pps_count)?;

        Some(Self {
            profile,
            compatibility,
            level,
            nal_length_size,
            sequence_parameter_sets,
            picture_parameter_sets,
        })
    }
}

fn read_parameter_sets(fields: &mut Fields<'_>, count: u8) -> Option<Vec<Vec<u8>>> {
    let mut sets = Vec::with_capacity(usize::from(count));
    for _ in 0..count {
        let length = usize::from(fields.u16("parameter set length").ok()?);
        sets.push(fields.take(length, "parameter set").ok()?.to_vec());
    }
    Some(sets)
}

/// The sample description: the entries a track's samples are decoded through.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stsd {
    /// The entry count the box declares.
    pub declared_count: u32,
    /// The entries as enumerated by size, which may differ from the declared count.
    pub entries: Vec<SampleEntry>,
}

impl Stsd {
    fn read(container: &Container, range: &BoxRange) -> Result<Self, Error> {
        let payload = container.payload_of(range).unwrap_or(&[]);
        let mut fields = Fields::new(payload, range.kind, range.offset);
        fields.version_and_flags()?;
        let declared_count = fields.u32("entry count")?;
        let entries = range
            .children
            .iter()
            .filter(|child| !child.clipped)
            .map(|child| SampleEntry::read(container, child))
            .collect();

        Ok(Self {
            declared_count,
            entries,
        })
    }
}

/// The sync sample table: which samples can be decoded without a predecessor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stss {
    /// One-based sample numbers, ascending.
    pub sample_numbers: Vec<u32>,
}

impl Stss {
    /// Reads the table from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the table is shorter than its count declares.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"stss");
        let mut fields = Fields::new(payload, kind, offset);
        fields.version_and_flags()?;
        let count = fields.u32("entry count")?;
        let mut sample_numbers = Vec::with_capacity(bounded_capacity(count));
        for _ in 0..count {
            sample_numbers.push(fields.u32("sample number")?);
        }
        Ok(Self { sample_numbers })
    }
}

/// The composition offset table.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Ctts {
    /// Runs of `(sample count, composition offset)`.
    pub entries: Vec<(u32, i64)>,
}

impl Ctts {
    /// Reads the table from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the table is shorter than its count declares.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"ctts");
        let mut fields = Fields::new(payload, kind, offset);
        let (version, _) = fields.version_and_flags()?;
        known_version(version, &[0, 1], kind, offset)?;
        let count = fields.u32("entry count")?;
        let mut entries = Vec::with_capacity(bounded_capacity(count));
        for _ in 0..count {
            let samples = fields.u32("sample count")?;
            let composition_offset = if version == 0 {
                i64::from(fields.u32("composition offset")?)
            } else {
                i64::from(fields.i32("composition offset")?)
            };
            entries.push((samples, composition_offset));
        }
        Ok(Self { entries })
    }
}

/// The edit list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Elst {
    /// Entries of `(segment duration in movie ticks, media time in media ticks, rate as 16.16)`.
    pub entries: Vec<(u64, i64, u32)>,
}

impl Elst {
    /// Reads the list from its payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the list is shorter than its count declares.
    pub fn read(payload: &[u8], offset: u64) -> Result<Self, Error> {
        let kind = FourCc(*b"elst");
        let mut fields = Fields::new(payload, kind, offset);
        let (version, _) = fields.version_and_flags()?;
        known_version(version, &[0, 1], kind, offset)?;
        let count = fields.u32("entry count")?;
        let mut entries = Vec::with_capacity(bounded_capacity(count));
        for _ in 0..count {
            let duration = fields.versioned(version, "segment duration")?;
            let media_time = if version == 0 {
                i64::from(fields.i32("media time")?)
            } else {
                fields.i64("media time")?
            };
            let rate = fields.u32("media rate")?;
            entries.push((duration, media_time, rate));
        }
        Ok(Self { entries })
    }
}

/// A capacity hint that a hostile count cannot turn into an allocation of gigabytes.
pub(crate) fn bounded_capacity(count: u32) -> usize {
    usize::try_from(count).unwrap_or(0).min(1 << 16)
}

/// A track, with everything needed to describe it and to find its samples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Track {
    /// The zero-based position of the track box under the movie box.
    pub index: usize,
    /// The track box as a range with its tree.
    pub range: BoxRange,
    /// The track header.
    pub tkhd: Tkhd,
    /// The media header.
    pub mdhd: Mdhd,
    /// The handler reference.
    pub hdlr: Hdlr,
    /// What the track carries.
    pub kind: TrackKind,
    /// The sample description.
    pub stsd: Stsd,
    /// The edit list, when the track has one.
    pub elst: Option<Elst>,
    /// The sample tables, decoded.
    pub table: SampleTable,
}

impl Track {
    /// Reads a track from its box.
    ///
    /// # Errors
    ///
    /// Returns an error when a required box is missing or malformed, or the tables contradict each other.
    pub fn read(container: &Container, trak: &BoxRange, index: usize) -> Result<Self, Error> {
        let required = |path: &[&[u8; 4]], field: &'static str| {
            trak.descend(path).ok_or(Error::Truncated {
                kind: trak.kind,
                offset: trak.offset,
                field,
            })
        };
        let payload = |range: &BoxRange| container.payload_of(range).unwrap_or(&[]);

        let tkhd_range = required(&[b"tkhd"], "track header")?;
        let tkhd = Tkhd::read(payload(tkhd_range), tkhd_range.offset)?;
        let mdhd_range = required(&[b"mdia", b"mdhd"], "media header")?;
        let mdhd = Mdhd::read(payload(mdhd_range), mdhd_range.offset)?;
        let hdlr_range = required(&[b"mdia", b"hdlr"], "handler reference")?;
        let hdlr = Hdlr::read(payload(hdlr_range), hdlr_range.offset)?;
        let stbl = required(&[b"mdia", b"minf", b"stbl"], "sample table")?;
        let stsd_range = stbl.child(b"stsd").ok_or(Error::Truncated {
            kind: stbl.kind,
            offset: stbl.offset,
            field: "sample description",
        })?;
        let stsd = Stsd::read(container, stsd_range)?;
        let elst = match trak.descend(&[b"edts", b"elst"]) {
            Some(range) => Some(Elst::read(payload(range), range.offset)?),
            None => None,
        };
        let table = SampleTable::read(container, stbl, tkhd.track_id)?;

        let kind = match &hdlr.handler_type.bytes() {
            b"vide" => TrackKind::Video,
            b"soun" => TrackKind::Audio,
            _ => TrackKind::Other(hdlr.handler_type),
        };

        Ok(Self {
            index,
            range: trak.clone(),
            tkhd,
            mdhd,
            hdlr,
            kind,
            stsd,
            elst,
            table,
        })
    }

    /// The first sample entry, which is the one nearly every file uses for every sample.
    #[must_use]
    pub fn entry(&self) -> Option<&SampleEntry> {
        self.stsd.entries.first()
    }

    /// The media duration in milliseconds, or nothing when the timescale is zero.
    #[must_use]
    pub fn duration_milliseconds(&self) -> Option<u64> {
        let ticks = u128::from(self.mdhd.duration).checked_mul(1000)?;
        let milliseconds = ticks.checked_div(u128::from(self.mdhd.timescale))?;
        u64::try_from(milliseconds).ok()
    }
}

#[cfg(test)]
mod tests {
    use super::{AvcConfiguration, Ctts, Elst, Fields, Ftyp, Hdlr, Mdhd, Mvhd, Stss, Tkhd, bounded_capacity};
    use crate::error::Error;
    use crate::fourcc::FourCc;

    #[test]
    fn a_file_type_box_lists_its_compatible_brands() {
        // Arrange
        let payload = b"mp42\0\0\0\0mp42mp41";

        // Act
        let ftyp = Ftyp::read(payload).unwrap();

        // Assert
        assert_eq!(ftyp.major_brand.to_string(), "mp42");
        assert_eq!(ftyp.compatible_brands.len(), 2);
        assert_eq!(
            ftyp.compatible_brands.get(1).map(ToString::to_string),
            Some("mp41".to_owned())
        );
    }

    #[test]
    fn a_version_one_movie_header_reads_its_wide_fields() {
        // Arrange
        let mut payload = vec![1, 0, 0, 0];
        payload.extend_from_slice(&7u64.to_be_bytes());
        payload.extend_from_slice(&8u64.to_be_bytes());
        payload.extend_from_slice(&60000u32.to_be_bytes());
        payload.extend_from_slice(&3_600_000u64.to_be_bytes());
        payload.extend_from_slice(&[0; 4 + 2 + 10 + 36 + 24]);
        payload.extend_from_slice(&4u32.to_be_bytes());

        // Act
        let mvhd = Mvhd::read(&payload, 0).unwrap();

        // Assert
        assert_eq!(mvhd.creation_time, 7);
        assert_eq!(mvhd.timescale, 60000);
        assert_eq!(mvhd.duration, 3_600_000);
        assert_eq!(mvhd.next_track_id, 4);
    }

    #[test]
    fn a_header_that_stops_short_names_the_missing_field() {
        // Arrange
        let payload = [0, 0, 0, 0, 0, 0, 0, 1];

        // Act
        let outcome = Tkhd::read(&payload, 40);

        // Assert
        assert!(
            matches!(
                outcome,
                Err(Error::Truncated {
                    offset: 40,
                    field: "modification time",
                    ..
                })
            ),
            "got {outcome:?}"
        );
    }

    #[test]
    fn an_unknown_version_is_refused_rather_than_guessed() {
        // Arrange
        let payload = [2, 0, 0, 0];

        // Act
        let outcome = Mvhd::read(&payload, 0);

        // Assert
        assert!(matches!(outcome, Err(Error::UnknownVersion { version: 2, .. })));
    }

    #[test]
    fn the_avc_configuration_yields_its_parameter_sets_and_length_size() {
        // Arrange
        let payload = [
            1, 0x64, 0x00, 0x28, 0xff, 0xe1, 0, 3, 0x67, 0xaa, 0xbb, 1, 0, 2, 0x68, 0xcc,
        ];

        // Act
        let avc = AvcConfiguration::read(&payload).unwrap();

        // Assert
        assert_eq!(avc.nal_length_size, 4);
        assert_eq!(avc.profile, 0x64);
        assert_eq!(avc.sequence_parameter_sets, vec![vec![0x67, 0xaa, 0xbb]]);
        assert_eq!(avc.picture_parameter_sets, vec![vec![0x68, 0xcc]]);
    }

    #[test]
    fn a_configuration_whose_parameter_set_overruns_is_nothing_rather_than_a_panic() {
        // Arrange
        let payload = [1, 0x64, 0x00, 0x28, 0xff, 0xe1, 0xff, 0xff, 0x67];

        // Act
        let read = AvcConfiguration::read(&payload);

        // Assert
        assert!(read.is_none());
    }

    #[test]
    fn a_configuration_of_another_version_is_nothing() {
        // Act
        let read = AvcConfiguration::read(&[2, 0x64, 0x00, 0x28, 0xff, 0xe0, 0]);

        // Assert
        assert!(read.is_none());
    }

    #[test]
    fn version_and_flags_are_read_as_one_byte_and_three() {
        // Arrange
        let mut fields = Fields::new(&[1, 0x01, 0x02, 0x03, 9], FourCc(*b"test"), 0);

        // Act
        let (version, flags) = fields.version_and_flags().unwrap();

        // Assert
        assert_eq!(version, 1);
        assert_eq!(flags, 0x0001_0203);
        assert_eq!(fields.rest(), &[9]);
    }

    #[test]
    fn signed_fields_are_read_in_two_complement() {
        // Arrange
        let mut fields = Fields::new(&[0xff, 0xff, 0xff, 0xff, 0xff, 0xfe], FourCc(*b"test"), 0);

        // Act
        let short = fields.i16("layer").unwrap();
        let wide = fields.i32("offset").unwrap();

        // Assert
        assert_eq!(short, -1);
        assert_eq!(wide, -2);
    }

    #[test]
    fn a_track_header_reads_its_flags_layer_and_dimensions() {
        // Arrange: version 0, flags 3, times 0, track 7, duration 0, layer -1, group 2, volume 0x0100,
        // width 1920 and height 1080 as 16.16.
        let mut payload = vec![0, 0, 0, 3];
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&7u32.to_be_bytes());
        payload.extend_from_slice(&[0; 4]);
        payload.extend_from_slice(&[0; 4]);
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&(-1i16).to_be_bytes());
        payload.extend_from_slice(&2i16.to_be_bytes());
        payload.extend_from_slice(&0x0100u16.to_be_bytes());
        payload.extend_from_slice(&[0; 2 + 36]);
        payload.extend_from_slice(&(1920u32 << 16).to_be_bytes());
        payload.extend_from_slice(&(1080u32 << 16).to_be_bytes());

        // Act
        let tkhd = Tkhd::read(&payload, 0).unwrap();

        // Assert
        assert_eq!(tkhd.track_id, 7);
        assert_eq!(tkhd.flags, 3);
        assert!(tkhd.is_enabled());
        assert_eq!(tkhd.layer, -1);
        assert_eq!(tkhd.alternate_group, 2);
        assert_eq!(tkhd.volume, 0x0100);
        assert_eq!(tkhd.width >> 16, 1920);
        assert_eq!(tkhd.height >> 16, 1080);
        let disabled = Tkhd {
            flags: Tkhd::IN_MOVIE | Tkhd::IN_PREVIEW,
            ..tkhd
        };
        assert!(!disabled.is_enabled());
    }

    #[test]
    fn a_media_header_decodes_its_packed_language() {
        // Arrange: version 0, timescale 30000, duration 0, language "eng" packed as 0x15c7.
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(&[0; 8]);
        payload.extend_from_slice(&30000u32.to_be_bytes());
        payload.extend_from_slice(&0u32.to_be_bytes());
        payload.extend_from_slice(&0x15c7u16.to_be_bytes());
        payload.extend_from_slice(&[0; 2]);

        // Act
        let mdhd = Mdhd::read(&payload, 0).unwrap();

        // Assert
        assert_eq!(mdhd.language, "eng");
        assert_eq!(mdhd.timescale, 30000);
    }

    #[test]
    fn composition_offsets_are_unsigned_in_version_zero_and_signed_in_version_one() {
        // Arrange
        let mut zero = vec![0, 0, 0, 0];
        zero.extend_from_slice(&1u32.to_be_bytes());
        zero.extend_from_slice(&5u32.to_be_bytes());
        zero.extend_from_slice(&0xffff_fffeu32.to_be_bytes());
        let mut one = vec![1, 0, 0, 0];
        one.extend_from_slice(&1u32.to_be_bytes());
        one.extend_from_slice(&5u32.to_be_bytes());
        one.extend_from_slice(&0xffff_fffeu32.to_be_bytes());

        // Act
        let unsigned = Ctts::read(&zero, 0).unwrap();
        let signed = Ctts::read(&one, 0).unwrap();

        // Assert
        assert_eq!(unsigned.entries, vec![(5, 4_294_967_294)]);
        assert_eq!(signed.entries, vec![(5, -2)]);
    }

    #[test]
    fn an_edit_list_reads_a_negative_media_time_in_both_versions() {
        // Arrange
        let mut zero = vec![0, 0, 0, 0];
        zero.extend_from_slice(&1u32.to_be_bytes());
        zero.extend_from_slice(&600u32.to_be_bytes());
        zero.extend_from_slice(&(-1i32).to_be_bytes());
        zero.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        let mut one = vec![1, 0, 0, 0];
        one.extend_from_slice(&1u32.to_be_bytes());
        one.extend_from_slice(&600u64.to_be_bytes());
        one.extend_from_slice(&(-1i64).to_be_bytes());
        one.extend_from_slice(&0x0001_0000u32.to_be_bytes());

        // Act
        let narrow = Elst::read(&zero, 0).unwrap();
        let wide = Elst::read(&one, 0).unwrap();

        // Assert
        assert_eq!(narrow.entries, vec![(600, -1, 0x0001_0000)]);
        assert_eq!(wide.entries, vec![(600, -1, 0x0001_0000)]);
    }

    #[test]
    fn a_sync_table_is_read_in_full_and_refused_when_short() {
        // Arrange
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(&2u32.to_be_bytes());
        payload.extend_from_slice(&1u32.to_be_bytes());
        payload.extend_from_slice(&16u32.to_be_bytes());

        // Act
        let stss = Stss::read(&payload, 0).unwrap();
        let short = Stss::read(&payload[..12], 0);

        // Assert
        assert_eq!(stss.sample_numbers, vec![1, 16]);
        assert!(matches!(
            short,
            Err(Error::Truncated {
                field: "sample number",
                ..
            })
        ));
    }

    #[test]
    fn a_capacity_hint_is_bounded() {
        // Act and assert
        assert_eq!(bounded_capacity(5), 5);
        assert_eq!(bounded_capacity(65_536), 65_536);
        assert_eq!(bounded_capacity(70_000), 65_536);
        assert_eq!(bounded_capacity(u32::MAX), 65_536);
    }

    #[test]
    fn a_handler_reads_its_type_and_trims_the_terminator_from_its_name() {
        // Arrange
        let mut payload = vec![0, 0, 0, 0];
        payload.extend_from_slice(&[0; 4]);
        payload.extend_from_slice(b"vide");
        payload.extend_from_slice(&[0; 12]);
        payload.extend_from_slice(b"Video Handler\0");

        // Act
        let hdlr = Hdlr::read(&payload, 0).unwrap();

        // Assert
        assert_eq!(hdlr.handler_type, b"vide");
        assert_eq!(hdlr.name, "Video Handler");
    }
}
