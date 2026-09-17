//! The manifest, version 1: what was seen, as data.
//!
//! Every field is a fact a stranger can recompute from the file with ordinary tools, or a fact about how
//! this record was made. Nothing in it is a conclusion. Unknown fields are refused on reading, so a
//! manifest from a later version is refused rather than half-read.

use adiungere_fingerprint::{AnnexBDigest, Digest, StructuralFingerprint, TrackFingerprint};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::time::TimeValue;

/// The identifier of this format, written into every manifest.
pub const MANIFEST_FORMAT: &str = "adiungere-manifest/1";

/// The manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// The format identifier, always [`MANIFEST_FORMAT`].
    pub format: String,
    /// How and when this record was made.
    pub produced: Produced,
    /// The file that was seen.
    pub file: FileRecord,
    /// Every track, in file order.
    pub tracks: Vec<TrackRecord>,
    /// The vendor boxes, preserved as bytes and digested, never interpreted.
    pub vendor: VendorRecord,
    /// Every time the file carries, each with its clock.
    pub times: Vec<TimeValue>,
}

/// How and when a manifest was made.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Produced {
    /// The tool's name.
    pub tool: String,
    /// The tool's version.
    pub version: String,
    /// The operating system and architecture the tool ran on.
    pub platform: String,
    /// When, by the system clock.
    pub at: TimeValue,
    /// What was done.
    pub operation: Operation,
}

/// What a manifest records having been done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Operation {
    /// The file was read and nothing was written.
    Inspection {
        /// How much was read.
        scope: Scope,
    },
    /// The file this manifest describes was written from the sources listed, by the core.
    Export {
        /// The class of the export, which is what every integrity sentence about it derives from.
        class: ExportClass,
        /// What was done to faces, plates and the burned-in strip, chosen explicitly and never defaulted.
        masking: Masking,
        /// The recordings the output was made from, with the fingerprints of the tracks taken from each.
        sources: Vec<SourceRecord>,
    },
}

/// The four classes an export can belong to, permanent labels that every surface derives its wording
/// from and nothing else.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ExportClass {
    /// Coded samples of some tracks copied byte for byte into a rebuilt container.
    Extraction,
    /// Every camera as its own video track plus the audio, in one rebuilt container, with no encoder.
    TwoTrackArchive,
    /// Composed and re-encoded; the default composition.
    RenditionLossy,
    /// Composed and re-encoded without loss, a label granted only after every frame was decoded and
    /// compared with its source.
    RenditionLosslessVerified,
}

/// What an export did to the people, the plates and the burned-in strip in the picture.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Masking {
    /// Nothing was masked: the file is for a claim or a court, and the recipient needs every detail.
    None,
    /// Faces and number plates were masked, in a re-encoded rendition.
    FacesAndPlates,
    /// Faces, number plates and the strip the recorder burns into the picture were masked.
    FacesPlatesAndStrip,
}

/// One recording an export was made from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceRecord {
    /// The file name, without its directory.
    pub name: String,
    /// The size in bytes.
    pub size: u64,
    /// The digest of the whole source, when it was read in full.
    pub sha256: Option<Digest>,
    /// The tracks taken from this source, each with the fingerprint it had there.
    pub tracks: Vec<SourceTrack>,
}

/// One track taken from a source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SourceTrack {
    /// The track's index under the source's movie box.
    pub index: usize,
    /// The track's index under the output's movie box.
    pub output_index: usize,
    /// The track's fingerprint in the source, which an extraction preserves and a rendition does not.
    pub fingerprint: Option<TrackFingerprint>,
}

/// How much of a file an inspection read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// The structure only: headers and the movie box, never the media.
    Structure,
    /// Every byte: the structure, then every sample for the fingerprints and the whole file for its digest.
    Full,
}

/// The file that was seen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FileRecord {
    /// The file name, without its directory.
    pub name: String,
    /// The size in bytes.
    pub size: u64,
    /// The digest of every byte, when the whole file was read.
    pub sha256: Option<Digest>,
    /// The container's shape.
    pub structure: Structure,
}

/// The container's shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Structure {
    /// The major brand, when the file has a file type box.
    pub brand: Option<String>,
    /// The compatible brands.
    pub compatible_brands: Vec<String>,
    /// The top-level box types in file order.
    pub top_level: Vec<String>,
    /// Whether the movie box precedes the first media data box; nothing when there is no media data box.
    pub moov_before_mdat: Option<bool>,
    /// Where the movie box starts.
    pub moov_offset: u64,
    /// How large the movie box is.
    pub moov_size: u64,
    /// The compressor names and handler names found, in order, without duplicates.
    pub encoder_tags: Vec<String>,
    /// How many bytes at the end of the file do not read as a box, or belong to a box that declares more
    /// than the file holds; nothing when the file ends with a well-formed box.
    pub clipped_tail_bytes: Option<u64>,
    /// The structural fingerprint.
    pub structural_fingerprint: StructuralFingerprint,
}

/// What a track carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackKind {
    /// Video.
    Video,
    /// Audio.
    Audio,
    /// Something else, named by its handler type.
    Other(String),
}

/// One entry of an edit list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EditEntry {
    /// The segment duration in movie ticks.
    pub segment_duration: u64,
    /// The media time in media ticks, negative for an empty edit.
    pub media_time: i64,
    /// The rate as 16.16 fixed point.
    pub media_rate: u32,
}

/// One track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrackRecord {
    /// The zero-based position under the movie box.
    pub index: usize,
    /// The identifier from the track header.
    pub id: u32,
    /// What the track carries.
    pub kind: TrackKind,
    /// The sample entry type, such as `avc1` or `mp4a`.
    pub coding: String,
    /// The codec string in the registered web form, when it can be derived.
    pub codec_string: Option<String>,
    /// The coded width in pixels, for video.
    pub width: Option<u16>,
    /// The coded height in pixels, for video.
    pub height: Option<u16>,
    /// The sample rate, for audio.
    pub sample_rate: Option<u32>,
    /// The channel count, for audio.
    pub channels: Option<u16>,
    /// The media timescale.
    pub timescale: u32,
    /// The media duration in ticks.
    pub duration_ticks: u64,
    /// The media duration in milliseconds.
    pub duration_milliseconds: Option<u64>,
    /// How many samples.
    pub sample_count: u32,
    /// How many sync samples.
    pub sync_sample_count: u32,
    /// Whether the track carries composition offsets.
    pub composition_offsets: bool,
    /// The track header flags.
    pub flags: u32,
    /// Whether the enabled flag is set.
    pub enabled: bool,
    /// The edit list, when the track has one.
    pub edit_list: Option<Vec<EditEntry>>,
    /// The average bit rate in bits per second, from sample bytes over media duration.
    pub average_bit_rate: Option<u64>,
    /// The track fingerprint, when the samples were read.
    pub fingerprint: Option<TrackFingerprint>,
    /// The elementary stream digest, for advanced video when the samples were read.
    pub elementary_stream: Option<AnnexBDigest>,
}

/// Where a vendor box sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VendorLocation {
    /// A child of the user-data box under the movie box.
    UserData,
    /// A top-level box that is not part of the standard.
    TopLevel,
}

/// One vendor box.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VendorBox {
    /// Where it sits.
    pub location: VendorLocation,
    /// Its type code.
    pub kind: String,
    /// Where it starts.
    pub offset: u64,
    /// Its size, header included.
    pub size: u64,
    /// Whether the type is one the base media file format or its 3GPP extension defines under the
    /// user-data box, such as the metadata box a common muxer writes. A standard box is preserved and
    /// digested like any other; it is not counted as a recorder's box when the vendor boxes are said to be
    /// present or absent.
    pub standard: bool,
    /// The digest of the whole box, header included, when its bytes were held.
    pub sha256: Option<Digest>,
}

/// The vendor boxes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VendorRecord {
    /// Every vendor box, in file order.
    pub boxes: Vec<VendorBox>,
    /// Whether any of them was interpreted. Always false at this version: they are bytes.
    pub interpreted: bool,
}

impl Manifest {
    /// The manifest in its canonical text form: the fields in declared order, two-space indentation,
    /// digests in lower-case hexadecimal, a trailing newline.
    ///
    /// # Errors
    ///
    /// Returns an error only if serialisation fails, which the types do not allow.
    pub fn to_canonical_json(&self) -> Result<String, serde_json::Error> {
        let mut text = serde_json::to_string_pretty(self)?;
        text.push('\n');
        Ok(text)
    }

    /// Reads a manifest from its text, refusing unknown fields and a format this version does not know.
    ///
    /// # Errors
    ///
    /// Returns the parse error, or a format mismatch as a parse error.
    pub fn from_json(text: &str) -> Result<Self, serde_json::Error> {
        let manifest: Self = serde_json::from_str(text)?;
        if manifest.format != MANIFEST_FORMAT {
            return Err(serde::de::Error::custom(format!(
                "the manifest is {:?}; this version reads {MANIFEST_FORMAT:?}",
                manifest.format
            )));
        }
        Ok(manifest)
    }
}
