//! SHA-256, spelt the way every digest utility spells it.

use std::fmt;
use std::str::FromStr;

use adiungere_isobmff::Source;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest as _, Sha256};

use crate::Error;

/// A SHA-256 digest.
///
/// Displayed and serialised as sixty-four lower-case hexadecimal characters, which is the form
/// `sha256sum` prints and the form a person types back.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(pub [u8; 32]);

impl Digest {
    /// Digests a byte string in one call.
    #[must_use]
    pub fn of(bytes: &[u8]) -> Self {
        Self(Sha256::digest(bytes).into())
    }

    /// The raw bytes.
    #[must_use]
    pub const fn bytes(&self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "Digest({self})")
    }
}

/// A digest written in a form that is not sixty-four hexadecimal characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestError {
    text: String,
}

impl fmt::Display for DigestError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{:?} is not a SHA-256 digest: sixty-four hexadecimal characters are expected",
            self.text
        )
    }
}

impl std::error::Error for DigestError {}

impl FromStr for Digest {
    type Err = DigestError;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let error = || DigestError {
            text: text.to_owned(),
        };
        if text.len() != 64 {
            return Err(error());
        }
        let mut bytes = [0u8; 32];
        for (slot, pair) in bytes.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
            let high = hex_value(pair[0]).ok_or_else(error)?;
            let low = hex_value(pair[1]).ok_or_else(error)?;
            *slot = (high << 4) | low;
        }
        Ok(Self(bytes))
    }
}

fn hex_value(character: u8) -> Option<u8> {
    match character {
        b'0'..=b'9' => Some(character - b'0'),
        b'a'..=b'f' => Some(character - b'a' + 10),
        b'A'..=b'F' => Some(character - b'A' + 10),
        _ => None,
    }
}

impl Serialize for Digest {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Digest {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for Digest {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Sha256Digest".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "A SHA-256 digest as sixty-four lower-case hexadecimal characters.",
            "pattern": "^[0-9a-f]{64}$"
        })
    }
}

/// How many bytes are read per step when digesting a whole file.
const CHUNK: u64 = 1 << 20;

/// Digests a whole source, streaming, and reports its length.
///
/// # Errors
///
/// Returns an error when the source cannot deliver a range.
pub fn file_digest<S: Source>(source: &mut S) -> Result<(Digest, u64), Error> {
    digest_in_steps(source, CHUNK)
}

/// Digests a whole source, `step` bytes at a time. The step is a parameter so that the walk across step
/// boundaries is tested with a small one.
fn digest_in_steps<S: Source>(source: &mut S, step: u64) -> Result<(Digest, u64), Error> {
    let length = source.length()?;
    let mut hasher = Sha256::new();
    let mut offset = 0u64;

    while offset < length {
        let this_step = (length - offset).min(step.max(1));
        let bytes = source.read_range(offset, usize::try_from(this_step).unwrap_or(usize::MAX))?;
        hasher.update(&bytes);
        offset = offset.saturating_add(this_step);
    }

    Ok((Digest(hasher.finalize().into()), length))
}

#[cfg(test)]
mod tests {
    use super::Digest;
    use adiungere_isobmff::SliceSource;

    #[test]
    fn the_empty_string_has_its_well_known_digest() {
        // Act
        let digest = Digest::of(b"");

        // Assert
        assert_eq!(
            digest.to_string(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn a_digest_round_trips_through_its_text_form() {
        // Arrange
        let text = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

        // Act
        let parsed: Digest = text.parse().unwrap();

        // Assert
        assert_eq!(parsed.to_string(), text);
        assert!("e3b0".parse::<Digest>().is_err());
        assert!(
            "g3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
                .parse::<Digest>()
                .is_err()
        );
    }

    #[test]
    fn upper_case_hexadecimal_is_read_and_every_nibble_lands_in_its_place() {
        // Arrange
        let text = "0123456789ABCDEFabcdef0123456789ABCDEFabcdef0123456789ABCDEFabcd";

        // Act
        let parsed: Digest = text.parse().unwrap();

        // Assert
        assert_eq!(
            parsed.bytes(),
            [
                0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67,
                0x89, 0xab, 0xcd, 0xef, 0xab, 0xcd, 0xef, 0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef,
                0xab, 0xcd
            ]
        );
        assert_eq!(parsed.to_string(), text.to_ascii_lowercase());
        assert_eq!(
            format!("{parsed:?}"),
            format!("Digest({})", text.to_ascii_lowercase())
        );
    }

    #[test]
    fn the_schema_of_a_digest_is_a_string_of_sixty_four_hexadecimal_characters() {
        // Act
        let schema = schemars::schema_for!(Digest);
        let text = serde_json::to_string(&schema).unwrap();

        // Assert
        assert!(text.contains("\"type\":\"string\""), "{text}");
        assert!(text.contains("^[0-9a-f]{64}$"), "{text}");
        assert!(text.contains("Sha256Digest"), "{text}");
    }

    #[test]
    fn a_whole_file_is_digested_one_mebibyte_at_a_time() {
        // Act and assert
        assert_eq!(super::CHUNK, 1_048_576);
    }

    #[test]
    fn a_refused_text_is_quoted_in_the_error() {
        // Act
        let error = "not a digest".parse::<Digest>().unwrap_err();

        // Assert
        assert_eq!(
            error.to_string(),
            "\"not a digest\" is not a SHA-256 digest: sixty-four hexadecimal characters are expected"
        );
    }

    #[test]
    fn a_digest_taken_in_small_steps_equals_the_one_shot_digest() {
        // Arrange
        let bytes: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();

        for step in [1, 7, 64, 999, 1000, 1001, 4096] {
            let mut source = SliceSource::new(&bytes);

            // Act
            let (stepped, length) = super::digest_in_steps(&mut source, step).unwrap();

            // Assert
            assert_eq!(stepped, Digest::of(&bytes), "step {step}");
            assert_eq!(length, 1000, "step {step}");
        }
    }

    #[test]
    fn an_empty_source_has_the_empty_digest_and_no_length() {
        // Arrange
        let mut source = SliceSource::new(&[]);

        // Act
        let (digest, length) = super::file_digest(&mut source).unwrap();

        // Assert
        assert_eq!(digest, Digest::of(b""));
        assert_eq!(length, 0);
    }

    /// The production chunk is one mebibyte; this walks across two boundaries with the real entry point.
    /// Hashing three megabytes under the interpreter of the memory model takes longer than the rest of the
    /// suite together, and the property it proves is the one the stepped test proves above, so it runs on
    /// the native target only.
    #[cfg(not(miri))]
    #[test]
    fn a_streamed_file_digest_equals_the_one_shot_digest() {
        // Arrange
        let bytes: Vec<u8> = (0..3_000_000u32).map(|i| (i % 251) as u8).collect();
        let mut source = SliceSource::new(&bytes);

        // Act
        let (streamed, length) = super::file_digest(&mut source).unwrap();

        // Assert
        assert_eq!(streamed, Digest::of(&bytes));
        assert_eq!(length, 3_000_000);
    }
}
