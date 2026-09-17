//! The track fingerprint, version 1.
//!
//! The digest of every sample's stored bytes, in decode order, concatenated with nothing between them;
//! and, separately, the digest of the decoder configuration's bytes. Length prefixes are part of the
//! stored bytes and are digested as stored, so the prefix width is recorded beside the digest: two files
//! holding the same pictures under different prefix widths have different fingerprints, and both are
//! correct, because the fingerprint is a claim about bytes and not about pictures.

use adiungere_isobmff::{Container, SampleTable, Source, Track};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::Error;
use crate::digest::Digest;

/// The identifier of this definition, written into every fingerprint so a reader knows which rule made it.
pub const TRACK_FINGERPRINT_VERSION: &str = "adiungere-track-fp/1";

/// A track's fingerprint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TrackFingerprint {
    /// The definition that produced this record.
    pub version: String,
    /// The zero-based position of the track under the movie box.
    pub track_index: usize,
    /// The track identifier from its header.
    pub track_id: u32,
    /// How many samples were digested.
    pub sample_count: u32,
    /// How many bytes were digested.
    pub sample_bytes: u64,
    /// The width of the length prefix on every unit, for a video track whose configuration declares one.
    pub nal_length_size: Option<u8>,
    /// The digest of every sample's stored bytes in decode order.
    pub payload_sha256: Digest,
    /// The type of the decoder configuration box that was digested.
    pub configuration_box: String,
    /// The digest of the decoder configuration box's payload, as stored.
    pub configuration_sha256: Digest,
}

/// Computes a track's fingerprint by reading every sample.
///
/// # Errors
///
/// Returns an error when the track has no decoder configuration, or a sample cannot be read.
pub fn track_fingerprint<S: Source>(
    source: &mut S,
    container: &Container,
    track: &Track,
) -> Result<TrackFingerprint, Error> {
    let entry = track
        .entry()
        .ok_or(Error::NoConfiguration { track: track.index })?;
    let configuration = entry
        .configuration
        .as_ref()
        .ok_or(Error::NoConfiguration { track: track.index })?;
    let configuration_payload = container
        .payload_of(configuration)
        .ok_or(Error::NoConfiguration { track: track.index })?;

    let mut hasher = Sha256::new();
    let mut sample_bytes = 0u64;
    let samples = track.table.samples()?;

    for sample in &samples {
        let bytes = SampleTable::read_sample(source, sample)?;
        hasher.update(&bytes);
        sample_bytes = sample_bytes.saturating_add(bytes.len() as u64);
    }

    Ok(TrackFingerprint {
        version: TRACK_FINGERPRINT_VERSION.to_owned(),
        track_index: track.index,
        track_id: track.tkhd.track_id,
        sample_count: u32::try_from(samples.len()).unwrap_or(u32::MAX),
        sample_bytes,
        nal_length_size: entry.avc_configuration(container).map(|avc| avc.nal_length_size),
        payload_sha256: Digest(hasher.finalize().into()),
        configuration_box: configuration.kind.to_string(),
        configuration_sha256: Digest::of(configuration_payload),
    })
}
