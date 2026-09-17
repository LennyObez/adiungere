//! What can go wrong while reading a file, each case naming where.

use std::fmt;

use crate::fourcc::FourCc;
use crate::source::SourceError;

/// A reading failure. Every variant carries enough to point at the file.
#[derive(Debug)]
pub enum Error {
    /// The source could not deliver the bytes asked for.
    Source(SourceError),
    /// The file does not start with a box header, so it is not a base media file at all.
    NotIsobmff,
    /// A box declares a size that runs past the end of the file, or past the end of its parent.
    Overrun {
        /// The box.
        kind: FourCc,
        /// Where the box starts.
        offset: u64,
        /// The size it declares.
        size: u64,
        /// The end it is not allowed to pass.
        limit: u64,
    },
    /// A box declares a size smaller than its own header.
    Undersized {
        /// The box.
        kind: FourCc,
        /// Where the box starts.
        offset: u64,
        /// The size it declares.
        size: u64,
    },
    /// A box that has to be read in full is larger than the reader is prepared to hold.
    Oversized {
        /// The box.
        kind: FourCc,
        /// Where the box starts.
        offset: u64,
        /// The size it declares.
        size: u64,
        /// The most the reader accepts for that box.
        cap: u64,
    },
    /// A typed view ends before its fields do.
    Truncated {
        /// The box.
        kind: FourCc,
        /// Where the box starts.
        offset: u64,
        /// Which field could not be read.
        field: &'static str,
    },
    /// A typed view declares a version this reader does not know.
    UnknownVersion {
        /// The box.
        kind: FourCc,
        /// Where the box starts.
        offset: u64,
        /// The version.
        version: u8,
    },
    /// Boxes are nested deeper than any real file nests them.
    TooDeep {
        /// Where the nesting exceeded the cap.
        offset: u64,
    },
    /// A container holds more children than any real file holds.
    TooMany {
        /// The container.
        kind: FourCc,
        /// Where it starts.
        offset: u64,
    },
    /// The file has no movie box, so there is nothing to describe.
    NoMovieBox,
    /// The file has two movie boxes, and this reader refuses to guess which one a player would use.
    TwoMovieBoxes {
        /// Where the second one starts.
        offset: u64,
    },
    /// A track's sample tables contradict each other.
    Inconsistent {
        /// The track, by identifier.
        track: u32,
        /// What was found.
        what: &'static str,
    },
    /// The driver was asked for a step it cannot take: bytes were fed when none were requested, or a
    /// request was made while one was outstanding.
    Protocol,
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Source(cause) => write!(formatter, "the source failed: {cause}"),
            Self::NotIsobmff => formatter.write_str("the file does not begin with a box header"),
            Self::Overrun {
                kind,
                offset,
                size,
                limit,
            } => write!(
                formatter,
                "the {kind} box at {offset} declares {size} bytes and would end past {limit}"
            ),
            Self::Undersized { kind, offset, size } => write!(
                formatter,
                "the {kind} box at {offset} declares {size} bytes, fewer than its own header"
            ),
            Self::Oversized {
                kind,
                offset,
                size,
                cap,
            } => write!(
                formatter,
                "the {kind} box at {offset} declares {size} bytes; this reader holds at most {cap}"
            ),
            Self::Truncated { kind, offset, field } => {
                write!(formatter, "the {kind} box at {offset} ends before its {field}")
            },
            Self::UnknownVersion {
                kind,
                offset,
                version,
            } => write!(
                formatter,
                "the {kind} box at {offset} is version {version}, which this reader does not know"
            ),
            Self::TooDeep { offset } => {
                write!(formatter, "boxes nest too deeply at {offset}")
            },
            Self::TooMany { kind, offset } => {
                write!(formatter, "the {kind} box at {offset} holds too many children")
            },
            Self::NoMovieBox => formatter.write_str("the file has no movie box"),
            Self::TwoMovieBoxes { offset } => {
                write!(formatter, "a second movie box starts at {offset}")
            },
            Self::Inconsistent { track, what } => {
                write!(formatter, "track {track}: {what}")
            },
            Self::Protocol => formatter.write_str("the parser was driven out of order"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<SourceError> for Error {
    fn from(cause: SourceError) -> Self {
        Self::Source(cause)
    }
}
