//! A time-stamp token over any bytes, independent of Content Credentials.
//!
//! A hash proves nothing about time; a time-stamping authority does, to the degree its status gives it.
//! This is the route for a manifest that carries no Content Credentials, and for anyone who wants an
//! authority's word on a file without a signing chain of their own: the bytes are hashed, the authority
//! is asked over the standard protocol, and the token it returns is kept beside the bytes. Checking a
//! token establishes that it is well formed, that its signature holds and that it names these bytes; it
//! does not establish, and this crate does not say, that the authority is on any trust list.

use c2pa::Context;
use c2pa::crypto::cose::CertificateTrustPolicy;
use c2pa::crypto::time_stamp::{default_rfc3161_message, default_rfc3161_request, verify_time_stamp};
use c2pa::status_tracker::StatusTracker;

use crate::error::Error;

/// A token an authority returned over some bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// The token, as the authority returned it.
    pub bytes: Vec<u8>,
    /// The authority asked.
    pub authority: String,
    /// The time the authority attested, as the token states it.
    pub time: String,
}

/// What a check of a token against some bytes established.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Attested {
    /// The time the authority attested.
    pub time: String,
    /// Whether the authority's chain was checked against a trust list: never, by this crate.
    pub trust_checked: bool,
}

/// Asks the authority for a token over the bytes, and checks the token before returning it.
///
/// # Errors
///
/// Returns an error when the request cannot be built, when the authority does not answer or answers with
/// something that is not a token over these bytes.
pub fn stamp(bytes: &[u8], authority: &str) -> Result<Token, Error> {
    let failure = |reason: String| Error::TimeStamp {
        authority: authority.to_owned(),
        reason,
    };
    let request = default_rfc3161_message(bytes).map_err(|cause| failure(cause.to_string()))?;
    let token = default_rfc3161_request(authority, None, &request, bytes, &Context::new())
        .map_err(|cause| failure(cause.to_string()))?;
    let attested = check(&token, bytes).map_err(|cause| failure(cause.to_string()))?;
    Ok(Token {
        bytes: token,
        authority: authority.to_owned(),
        time: attested.time,
    })
}

/// Checks a token against the bytes it is said to be over.
///
/// # Errors
///
/// Returns an error when the token does not parse, when its signature does not hold, or when it is not
/// over these bytes.
pub fn check(token: &[u8], bytes: &[u8]) -> Result<Attested, Error> {
    let mut log = StatusTracker::default();
    let info = verify_time_stamp(
        token,
        bytes,
        &CertificateTrustPolicy::passthrough(),
        &mut log,
        false,
    )
    .map_err(|cause| Error::TokenMismatch {
        reason: cause.to_string(),
    })?;
    Ok(Attested {
        time: info.gen_time.to_string(),
        trust_checked: false,
    })
}
