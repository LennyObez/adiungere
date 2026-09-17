//! Four-character box and entry types.

use std::fmt;

use serde::{Serialize, Serializer};

/// A four-byte type code, as it appears in a box header or a sample entry.
///
/// Kept as the raw bytes rather than as text, because a vendor is free to use bytes that are not printable
/// and the code still has to round-trip. The display form escapes anything that is not printable ASCII.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourCc(pub [u8; 4]);

impl FourCc {
    /// The code as the four bytes it is stored as.
    #[must_use]
    pub const fn bytes(self) -> [u8; 4] {
        self.0
    }

    /// Whether every byte is printable ASCII, which is what a well-behaved writer uses.
    #[must_use]
    pub fn is_printable(self) -> bool {
        self.0.iter().all(|byte| (0x20..=0x7e).contains(byte))
    }

    /// Whether this is a type the base media file format or its 3GPP extension defines as a child of the
    /// user-data box. Any other child of the user-data box is a vendor's.
    ///
    /// The reader still keeps every child as opaque bytes; this only says whether the type is one a
    /// standard names, so that a metadata box a common muxer writes is not mistaken for a recorder's
    /// telemetry.
    #[must_use]
    pub fn is_standard_user_data(self) -> bool {
        STANDARD_USER_DATA.contains(&&self.0)
    }
}

/// The child types of the user-data box that ISO/IEC 14496-12 (copyright, track selection, sub-track,
/// kind, metadata, hint information) and 3GPP TS 26.244 (title, description, performer, author, genre,
/// rating, classification, keywords, location, album, recording year, user rating, thumbnail) define.
const STANDARD_USER_DATA: [&[u8; 4]; 21] = [
    b"cprt", b"tsel", b"strk", b"kind", b"meta", b"hnti", b"hinf", b"titl", b"dscp", b"perf", b"auth",
    b"gnre", b"rtng", b"clsf", b"kywd", b"loci", b"albm", b"yrrc", b"urat", b"thmb", b"name",
];

impl From<&[u8; 4]> for FourCc {
    fn from(bytes: &[u8; 4]) -> Self {
        Self(*bytes)
    }
}

impl From<[u8; 4]> for FourCc {
    fn from(bytes: [u8; 4]) -> Self {
        Self(bytes)
    }
}

impl PartialEq<&[u8; 4]> for FourCc {
    fn eq(&self, other: &&[u8; 4]) -> bool {
        &self.0 == *other
    }
}

impl PartialEq<[u8; 4]> for FourCc {
    fn eq(&self, other: &[u8; 4]) -> bool {
        &self.0 == other
    }
}

impl fmt::Display for FourCc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &byte in &self.0 {
            if (0x20..=0x7e).contains(&byte) {
                write!(formatter, "{}", char::from(byte))?;
            } else {
                write!(formatter, "\\x{byte:02x}")?;
            }
        }
        Ok(())
    }
}

impl fmt::Debug for FourCc {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "FourCc({self})")
    }
}

impl Serialize for FourCc {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

#[cfg(test)]
mod tests {
    use super::FourCc;

    #[test]
    fn a_printable_code_displays_as_its_letters() {
        // Act
        let shown = FourCc(*b"moov").to_string();

        // Assert
        assert_eq!(shown, "moov");
    }

    #[test]
    fn a_byte_outside_printable_ascii_is_escaped_rather_than_dropped() {
        // Act
        let shown = FourCc([0xa9, b'n', b'a', b'm']).to_string();

        // Assert
        assert_eq!(shown, "\\xa9nam");
        assert!(!FourCc([0xa9, b'n', b'a', b'm']).is_printable());
    }

    #[test]
    fn the_user_data_children_a_standard_defines_are_told_from_a_vendors() {
        // Act and assert
        assert!(FourCc(*b"meta").is_standard_user_data());
        assert!(FourCc(*b"cprt").is_standard_user_data());
        assert!(FourCc(*b"loci").is_standard_user_data());
        assert!(!FourCc(*b"zvnd").is_standard_user_data());
        assert!(!FourCc([0xa9, b'r', b'e', b'q']).is_standard_user_data());
        assert!(!FourCc(*b"mamt").is_standard_user_data());
    }

    #[test]
    fn a_code_compares_with_a_literal() {
        // Act and assert
        assert_eq!(FourCc(*b"mdat"), b"mdat");
        assert_ne!(FourCc(*b"mdat"), b"moov");
        assert_eq!(FourCc(*b"mdat"), *b"mdat");
        assert_ne!(FourCc(*b"mdat"), *b"moov");
        assert!(FourCc(*b"mdat").is_printable());
        assert_eq!(format!("{:?}", FourCc(*b"free")), "FourCc(free)");
        assert_eq!(FourCc::from(b"ftyp").bytes(), *b"ftyp");
    }
}
