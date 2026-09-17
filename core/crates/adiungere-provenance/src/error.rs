//! What can stop a signing, a time stamp or a reading of credentials.

use std::path::PathBuf;

/// Anything that stops this crate before it can answer.
#[derive(Debug)]
pub enum Error {
    /// The provenance library refused or failed.
    Library {
        /// What it said.
        cause: c2pa::Error,
    },
    /// A file could not be read or written.
    Io {
        /// The path.
        path: PathBuf,
        /// What the operating system said.
        cause: std::io::Error,
    },
    /// A path this crate would write names a file it reads.
    Overwrite {
        /// The path that would be written.
        path: PathBuf,
    },
    /// The facts to carry could not be turned into an assertion.
    Facts {
        /// What went wrong.
        cause: serde_json::Error,
    },
    /// The credential could not be built from what was given.
    Credential {
        /// Why.
        reason: String,
    },
    /// The time-stamping authority did not answer with a token this crate accepts.
    TimeStamp {
        /// The authority's address.
        authority: String,
        /// What went wrong.
        reason: String,
    },
    /// A time-stamp token does not match the bytes it is presented with.
    TokenMismatch {
        /// Why.
        reason: String,
    },
    /// The file carries no credentials, embedded or beside it.
    NoCredentials {
        /// The path.
        path: PathBuf,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Library { cause } => write!(formatter, "the provenance library failed: {cause}"),
            Self::Io { path, cause } => write!(formatter, "{}: {cause}", path.display()),
            Self::Overwrite { path } => write!(
                formatter,
                "{} is a file being read; nothing was written",
                path.display()
            ),
            Self::Facts { cause } => write!(formatter, "the facts could not be carried: {cause}"),
            Self::Credential { reason } => write!(formatter, "the credential cannot be used: {reason}"),
            Self::TimeStamp { authority, reason } => {
                write!(formatter, "no time stamp from {authority}: {reason}")
            },
            Self::TokenMismatch { reason } => {
                write!(formatter, "the time-stamp token does not match: {reason}")
            },
            Self::NoCredentials { path } => write!(
                formatter,
                "{} carries no Content Credentials, embedded or beside it",
                path.display()
            ),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Library { cause } => Some(cause),
            Self::Io { cause, .. } => Some(cause),
            Self::Facts { cause } => Some(cause),
            _ => None,
        }
    }
}

impl From<c2pa::Error> for Error {
    fn from(cause: c2pa::Error) -> Self {
        Self::Library { cause }
    }
}
