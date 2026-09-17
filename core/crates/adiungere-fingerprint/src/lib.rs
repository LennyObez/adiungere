//! Fingerprints: numbers a stranger can reproduce with ordinary tools.
//!
//! An extraction never has the digest of its source, because the container around the samples is
//! rebuilt. So the integrity claim is made about what survives an extraction: the stored bytes of each
//! track's samples, in decode order. That is the **track fingerprint**, version 1, defined in
//! `docs/integrity.md` with a reference implementation short enough to read in one sitting.
//!
//! Beside it, three more numbers. The **elementary stream digest** is the digest of the same samples in
//! the start-coded form the common command-line tool writes, so that a person with that tool and a
//! digest utility can check a track without any code from this project. The **file digest** is the
//! digest of the whole file, which is what proves an original is the one that was seen. The
//! **structural fingerprint** digests the shape of the container rather than its media: the brand, the
//! box order, the encoder tags, the vendor boxes; two files from one recorder share it, and a file that was
//! rewritten by another program does not.
//!
//! Every one of these is a digest of bytes read from the file, never of a value this crate computed. That
//! is what makes them reproducible from outside.

#![forbid(unsafe_code)]

mod annex_b;
mod digest;
mod structural;
mod track;

pub use annex_b::{ANNEX_B_RULE, AnnexBDigest, annex_b_digest};
pub use digest::{Digest, DigestError, file_digest};
pub use structural::{StructuralFingerprint, structural_fingerprint};
pub use track::{TRACK_FINGERPRINT_VERSION, TrackFingerprint, track_fingerprint};

use std::fmt;

/// Why a fingerprint could not be computed.
#[derive(Debug)]
pub enum Error {
    /// The container could not be read.
    Container(adiungere_isobmff::Error),
    /// The track has no decoder configuration this fingerprint needs.
    NoConfiguration {
        /// The track, by index.
        track: usize,
    },
    /// The track is not a video track in the coding the elementary stream digest is defined for.
    NotAdvancedVideo {
        /// The track, by index.
        track: usize,
    },
    /// A sample does not parse into units under the declared length prefix.
    MalformedSample {
        /// The track, by index.
        track: usize,
        /// The sample, by index.
        sample: u32,
        /// What was found.
        what: &'static str,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(cause) => write!(formatter, "{cause}"),
            Self::NoConfiguration { track } => {
                write!(formatter, "track {track} carries no decoder configuration")
            },
            Self::NotAdvancedVideo { track } => write!(
                formatter,
                "track {track} is not advanced video coding, for which the stream digest is defined"
            ),
            Self::MalformedSample { track, sample, what } => {
                write!(formatter, "track {track}, sample {sample}: {what}")
            },
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Container(cause) => Some(cause),
            _ => None,
        }
    }
}

impl From<adiungere_isobmff::Error> for Error {
    fn from(cause: adiungere_isobmff::Error) -> Self {
        Self::Container(cause)
    }
}

impl From<adiungere_isobmff::SourceError> for Error {
    fn from(cause: adiungere_isobmff::SourceError) -> Self {
        Self::Container(adiungere_isobmff::Error::Source(cause))
    }
}
