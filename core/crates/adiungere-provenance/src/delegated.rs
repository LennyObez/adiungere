//! A credential whose key is elsewhere.
//!
//! The manifest is built where the media is, and the signature is made where the key is. What travels
//! between the two is the byte string to sign: the claim's signature structure, a few kilobytes that
//! name the assertions by their digests and carry none of the media. A service holding the key can read
//! that structure, decide whether it will put its name to it, and answer with a signature. This is the
//! shape the signing service of the website takes, and it is measured here, before that service exists,
//! by signing through a function that records exactly what it was given.

use c2pa::{Signer, SigningAlg};

use crate::credential::Algorithm;

/// A signer that hands the bytes to sign to a function and returns what it answers.
pub struct Delegated<F> {
    chain: Vec<Vec<u8>>,
    algorithm: Algorithm,
    reserve: usize,
    time_authority: Option<String>,
    sign: F,
}

impl<F> std::fmt::Debug for Delegated<F> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Delegated")
            .field("algorithm", &self.algorithm)
            .field("certificates", &self.chain.len())
            .field("time_authority", &self.time_authority)
            .finish_non_exhaustive()
    }
}

impl<F> Delegated<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, String>,
{
    /// A delegated signer for a chain in DER, end entity first, whose key signs with `algorithm` through
    /// `sign`. `reserve` is the largest signature the key produces, in bytes.
    pub fn new(chain: Vec<Vec<u8>>, algorithm: Algorithm, reserve: usize, sign: F) -> Self {
        Self {
            chain,
            algorithm,
            reserve,
            time_authority: None,
            sign,
        }
    }

    /// The same signer, asking this authority for a time stamp at every signing.
    #[must_use]
    pub fn stamped_by(mut self, authority: &str) -> Self {
        self.time_authority = Some(authority.to_owned());
        self
    }
}

impl<F> Signer for Delegated<F>
where
    F: Fn(&[u8]) -> Result<Vec<u8>, String>,
{
    fn sign(&self, data: &[u8]) -> c2pa::Result<Vec<u8>> {
        (self.sign)(data).map_err(|reason| c2pa::Error::OtherError(reason.into()))
    }

    fn alg(&self) -> SigningAlg {
        self.algorithm.library()
    }

    fn certs(&self) -> c2pa::Result<Vec<Vec<u8>>> {
        Ok(self.chain.clone())
    }

    fn reserve_size(&self) -> usize {
        self.reserve.saturating_add(16 * 1024)
    }

    fn time_authority_url(&self) -> Option<String> {
        self.time_authority.clone()
    }
}
