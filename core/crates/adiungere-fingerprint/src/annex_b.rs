//! The elementary stream digest: the track as the common command-line tool writes it.
//!
//! Copying a video track to a raw elementary stream turns length prefixes into start codes and inserts
//! the parameter sets from the decoder configuration in front of each instantaneous refresh picture. The
//! rule below reproduces that output byte for byte, which probe P34 measured, so that the digest of the
//! stream can be checked with the tool and a digest utility and nothing else.
//!
//! This is the *secondary* digest. It is related to the track fingerprint and never equal to it, because
//! the two digest different byte strings: one the samples as stored, the other the samples as a tool
//! rewrites them. Both are published, and neither is ever presented as the other.
//!
//! The rule, as `docs/integrity.md` states it:
//!
//! 1. Samples are taken in decode order. Each sample is split into units by its length prefixes.
//! 2. Within a sample, a unit that opens the sample's output gets a four-byte start code; every later
//!    unit gets a three-byte one. Inserted parameter sets count as output.
//! 3. Before the first refresh slice (type 5) of a refresh picture, unless a sequence or picture parameter
//!    set already appeared in the same sample, every sequence parameter set and then every picture
//!    parameter set from the decoder configuration is written, each with a four-byte start code. If only a
//!    sequence parameter set appeared in the sample, the picture parameter sets alone are written.
//! 4. A refresh picture is "new" at the start of the stream, after any parameter set unit, and at any
//!    refresh slice whose first macroblock is zero once a previous refresh picture has had its insertion.

use adiungere_isobmff::{AvcConfiguration, Container, SampleTable, Source, Track};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::Error;
use crate::digest::Digest;

/// The identifier of this rule, written into every digest so a reader knows which rule made it.
pub const ANNEX_B_RULE: &str = "adiungere-annexb/1";

/// The digest of a track's elementary stream.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnnexBDigest {
    /// The rule that produced this record.
    pub rule: String,
    /// The zero-based position of the track under the movie box.
    pub track_index: usize,
    /// How many bytes the stream holds.
    pub stream_bytes: u64,
    /// How many access units, which is how many samples, the stream holds.
    pub access_units: u32,
    /// How many times the parameter sets were inserted.
    pub parameter_set_insertions: u32,
    /// The digest of the stream.
    pub sha256: Digest,
}

const FOUR_BYTE_START: [u8; 4] = [0, 0, 0, 1];
const THREE_BYTE_START: [u8; 3] = [0, 0, 1];

/// The stream in its start-coded form, fed to a hasher unit by unit.
struct Writer {
    hasher: Sha256,
    bytes: u64,
    insertions: u32,
    new_refresh: bool,
    picture_bytes: u64,
}

impl Writer {
    fn new() -> Self {
        Self {
            hasher: Sha256::new(),
            bytes: 0,
            insertions: 0,
            new_refresh: true,
            picture_bytes: 0,
        }
    }

    fn emit(&mut self, unit: &[u8], forced_four: bool) {
        let start: &[u8] = if forced_four || self.picture_bytes == 0 {
            &FOUR_BYTE_START
        } else {
            &THREE_BYTE_START
        };
        self.hasher.update(start);
        self.hasher.update(unit);
        let written = (start.len() as u64).saturating_add(unit.len() as u64);
        self.bytes = self.bytes.saturating_add(written);
        self.picture_bytes = self.picture_bytes.saturating_add(written);
    }

    fn parameter_sets(&mut self, sets: &[Vec<u8>]) {
        for set in sets {
            self.emit(set, true);
        }
    }

    /// One access unit, as the tool's filter rewrites it.
    fn access_unit(&mut self, units: &[&[u8]], configuration: &AvcConfiguration) {
        let mut sps_seen = false;
        let mut pps_seen = false;
        self.picture_bytes = 0;

        for unit in units {
            let unit_type = unit.first().copied().unwrap_or(0) & 0x1f;

            match unit_type {
                7 => {
                    sps_seen = true;
                    self.new_refresh = true;
                },
                8 => {
                    pps_seen = true;
                    self.new_refresh = true;
                },
                _ => {},
            }

            let opens_picture = unit.get(1).is_some_and(|byte| byte & 0x80 != 0);
            if !self.new_refresh && unit_type == 5 && opens_picture {
                self.new_refresh = true;
            }

            if self.new_refresh && unit_type == 5 && !sps_seen && !pps_seen {
                self.parameter_sets(&configuration.sequence_parameter_sets);
                self.parameter_sets(&configuration.picture_parameter_sets);
                self.insertions = self.insertions.saturating_add(1);
                self.new_refresh = false;
            } else if self.new_refresh && unit_type == 5 && sps_seen && !pps_seen {
                self.parameter_sets(&configuration.picture_parameter_sets);
            }

            self.emit(unit, false);
        }
    }
}

/// Splits a stored sample into its units by the length prefix the configuration declares.
fn units_of(sample: &[u8], nal_length_size: u8) -> Option<Vec<&[u8]>> {
    let width = usize::from(nal_length_size);
    let mut units = Vec::new();
    let mut at = 0usize;

    while at < sample.len() {
        let prefix = sample.get(at..at.checked_add(width)?)?;
        // The prefix is a big-endian number of one to four bytes: right-aligned into four, it is the
        // number itself.
        let mut padded = [0u8; 4];
        padded
            .get_mut(4usize.checked_sub(width)?..)?
            .copy_from_slice(prefix);
        let length = usize::try_from(u32::from_be_bytes(padded)).ok()?;
        let start = at.checked_add(width)?;
        let end = start.checked_add(length)?;
        units.push(sample.get(start..end)?);
        at = end;
    }

    Some(units)
}

/// Computes the elementary stream digest of a video track by reading every sample.
///
/// # Errors
///
/// Returns an error when the track is not advanced video coding with a decoder configuration, when a
/// sample does not split into units under the declared prefix, or when a sample cannot be read.
pub fn annex_b_digest<S: Source>(
    source: &mut S,
    container: &Container,
    track: &Track,
) -> Result<AnnexBDigest, Error> {
    let configuration = track
        .entry()
        .and_then(|entry| entry.avc_configuration(container))
        .ok_or(Error::NotAdvancedVideo { track: track.index })?;

    let mut writer = Writer::new();
    let samples = track.table.samples()?;

    for sample in &samples {
        let bytes = SampleTable::read_sample(source, sample)?;
        let units = units_of(&bytes, configuration.nal_length_size).ok_or(Error::MalformedSample {
            track: track.index,
            sample: sample.index,
            what: "a unit's length prefix runs past the sample",
        })?;
        writer.access_unit(&units, &configuration);
    }

    Ok(AnnexBDigest {
        rule: ANNEX_B_RULE.to_owned(),
        track_index: track.index,
        stream_bytes: writer.bytes,
        access_units: u32::try_from(samples.len()).unwrap_or(u32::MAX),
        parameter_set_insertions: writer.insertions,
        sha256: Digest(writer.hasher.finalize().into()),
    })
}

#[cfg(test)]
mod tests {
    use super::{Writer, units_of};
    use adiungere_isobmff::AvcConfiguration;
    use sha2::Digest as _;

    fn configuration() -> AvcConfiguration {
        AvcConfiguration {
            profile: 0x64,
            compatibility: 0,
            level: 0x28,
            nal_length_size: 4,
            sequence_parameter_sets: vec![vec![0x67, 0xaa]],
            picture_parameter_sets: vec![vec![0x68, 0xbb]],
        }
    }

    fn stream_of(pictures: &[Vec<&[u8]>]) -> (Vec<u8>, u32) {
        // The expected form is assembled by hand here, so the test does not restate the writer.
        let mut writer = Writer::new();
        for units in pictures {
            writer.access_unit(units, &configuration());
        }
        let digest = writer.hasher.finalize();
        (digest.to_vec(), writer.insertions)
    }

    #[test]
    fn a_refresh_picture_gets_the_parameter_sets_then_a_three_byte_start_code() {
        // Arrange
        let refresh: &[u8] = &[0x65, 0x88, 0x01];
        let mut expected = Vec::new();
        expected.extend_from_slice(&[0, 0, 0, 1, 0x67, 0xaa]);
        expected.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xbb]);
        expected.extend_from_slice(&[0, 0, 1, 0x65, 0x88, 0x01]);

        // Act
        let (digest, insertions) = stream_of(&[vec![refresh]]);

        // Assert
        assert_eq!(digest, sha2::Sha256::digest(&expected).to_vec());
        assert_eq!(insertions, 1);
    }

    #[test]
    fn an_ordinary_picture_of_one_unit_is_a_four_byte_start_code_and_the_unit() {
        // Arrange
        let refresh: &[u8] = &[0x65, 0x88];
        let ordinary: &[u8] = &[0x41, 0x9a];
        let mut expected = Vec::new();
        expected.extend_from_slice(&[
            0, 0, 0, 1, 0x67, 0xaa, 0, 0, 0, 1, 0x68, 0xbb, 0, 0, 1, 0x65, 0x88,
        ]);
        expected.extend_from_slice(&[0, 0, 0, 1, 0x41, 0x9a]);

        // Act
        let (digest, _) = stream_of(&[vec![refresh], vec![ordinary]]);

        // Assert
        assert_eq!(digest, sha2::Sha256::digest(&expected).to_vec());
    }

    #[test]
    fn later_slices_of_one_picture_get_three_byte_start_codes_and_no_second_insertion() {
        // Arrange
        let first: &[u8] = &[0x65, 0x88];
        let second: &[u8] = &[0x65, 0x10];
        let mut expected = Vec::new();
        expected.extend_from_slice(&[
            0, 0, 0, 1, 0x67, 0xaa, 0, 0, 0, 1, 0x68, 0xbb, 0, 0, 1, 0x65, 0x88,
        ]);
        expected.extend_from_slice(&[0, 0, 1, 0x65, 0x10]);

        // Act
        let (digest, insertions) = stream_of(&[vec![first, second]]);

        // Assert
        assert_eq!(digest, sha2::Sha256::digest(&expected).to_vec());
        assert_eq!(insertions, 1);
    }

    #[test]
    fn two_refresh_pictures_in_a_row_each_get_an_insertion() {
        // Act
        let (_, insertions) = stream_of(&[vec![&[0x65, 0x88]], vec![&[0x65, 0x88]]]);

        // Assert
        assert_eq!(insertions, 2);
    }

    #[test]
    fn in_band_parameter_sets_suppress_the_insertion() {
        // Arrange
        let sps: &[u8] = &[0x67, 0x11];
        let pps: &[u8] = &[0x68, 0x22];
        let refresh: &[u8] = &[0x65, 0x88];
        let mut expected = Vec::new();
        expected.extend_from_slice(&[0, 0, 0, 1, 0x67, 0x11, 0, 0, 1, 0x68, 0x22, 0, 0, 1, 0x65, 0x88]);

        // Act
        let (digest, insertions) = stream_of(&[vec![sps, pps, refresh]]);

        // Assert
        assert_eq!(digest, sha2::Sha256::digest(&expected).to_vec());
        assert_eq!(insertions, 0);
    }

    #[test]
    fn an_in_band_sequence_parameter_set_alone_gets_only_the_picture_parameter_set_inserted() {
        // Arrange
        let sps: &[u8] = &[0x67, 0x11];
        let refresh: &[u8] = &[0x65, 0x88];
        let mut expected = Vec::new();
        expected.extend_from_slice(&[0, 0, 0, 1, 0x67, 0x11]);
        expected.extend_from_slice(&[0, 0, 0, 1, 0x68, 0xbb]);
        expected.extend_from_slice(&[0, 0, 1, 0x65, 0x88]);

        // Act
        let (digest, insertions) = stream_of(&[vec![sps, refresh]]);

        // Assert
        assert_eq!(digest, sha2::Sha256::digest(&expected).to_vec());
        assert_eq!(insertions, 0);
    }

    #[test]
    fn units_are_split_by_their_length_prefixes() {
        // Act
        let units = units_of(&[0, 0, 0, 2, 0x65, 0x88, 0, 0, 0, 1, 0x41], 4).unwrap();

        // Assert
        assert_eq!(units, vec![&[0x65, 0x88][..], &[0x41][..]]);
        assert!(units_of(&[0, 0, 0, 9, 0x65], 4).is_none());
        assert_eq!(units_of(&[1, 0x41], 1).unwrap(), vec![&[0x41][..]]);
    }

    #[test]
    fn a_length_prefix_is_read_as_a_big_endian_number_over_every_byte_of_its_width() {
        // Arrange: one unit of 258 bytes under a two-byte prefix, then one of 3 bytes.
        let mut sample = vec![0x01, 0x02];
        sample.extend(std::iter::repeat_n(0x41, 258));
        sample.extend_from_slice(&[0x00, 0x03, 0x65, 0x88, 0x99]);

        // Act
        let units = units_of(&sample, 2).unwrap();

        // Assert
        assert_eq!(units.len(), 2);
        assert_eq!(units.first().unwrap().len(), 258);
        assert_eq!(units.get(1).unwrap(), &&[0x65, 0x88, 0x99][..]);
        assert!(
            units_of(&[0x02, 0x01, 0x41], 2).is_none(),
            "513 bytes are declared and 1 is held"
        );
    }
}
