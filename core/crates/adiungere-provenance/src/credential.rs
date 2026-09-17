//! The credential a manifest is signed with, and the authority that stamps the time.
//!
//! Two kinds exist. A **held** credential is a certificate chain and its key, in PEM, given by whoever
//! operates the signing; what its signature is worth depends on whether the chain leads to the Content
//! Credentials trust list, which is the validator's finding and never this crate's claim. An **ephemeral**
//! credential is generated for the process and thrown away with it: its chain is self-signed, it is on no
//! trust list, and every surface that shows a signature made with it says so. It exists so that the whole
//! pipeline runs, and is watched, without a certificate anyone has to keep.

use c2pa::{EphemeralSigner, Signer, SigningAlg};

use crate::error::Error;

/// The signature algorithm a held credential's key is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// ECDSA over P-256 with SHA-256.
    Es256,
    /// ECDSA over P-384 with SHA-384.
    Es384,
    /// ECDSA over P-521 with SHA-512.
    Es512,
    /// RSA-PSS with SHA-256.
    Ps256,
    /// RSA-PSS with SHA-384.
    Ps384,
    /// RSA-PSS with SHA-512.
    Ps512,
    /// Edwards-curve signatures over Curve25519.
    Ed25519,
}

impl Algorithm {
    pub(crate) const fn library(self) -> SigningAlg {
        match self {
            Self::Es256 => SigningAlg::Es256,
            Self::Es384 => SigningAlg::Es384,
            Self::Es512 => SigningAlg::Es512,
            Self::Ps256 => SigningAlg::Ps256,
            Self::Ps384 => SigningAlg::Ps384,
            Self::Ps512 => SigningAlg::Ps512,
            Self::Ed25519 => SigningAlg::Ed25519,
        }
    }
}

impl std::str::FromStr for Algorithm {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        match text.to_ascii_lowercase().as_str() {
            "es256" => Ok(Self::Es256),
            "es384" => Ok(Self::Es384),
            "es512" => Ok(Self::Es512),
            "ps256" => Ok(Self::Ps256),
            "ps384" => Ok(Self::Ps384),
            "ps512" => Ok(Self::Ps512),
            "ed25519" => Ok(Self::Ed25519),
            other => Err(format!(
                "{other} is not a signature algorithm this product signs with (es256, es384, es512, ps256, \
                 ps384, ps512, ed25519)"
            )),
        }
    }
}

/// Which kind of credential a signature was made with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    /// A chain and a key given by the operator.
    Held,
    /// A self-signed chain generated for this process; on no trust list.
    Ephemeral,
}

/// A credential and, when one is configured, the time-stamping authority asked at every signing.
pub struct Credential {
    signer: Box<dyn Signer + Send + Sync>,
    kind: Kind,
    time_authority: Option<String>,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Credential")
            .field("kind", &self.kind)
            .field("time_authority", &self.time_authority)
            .finish_non_exhaustive()
    }
}

impl Credential {
    /// A self-signed chain generated now, named by `common_name`, on no trust list.
    ///
    /// # Errors
    ///
    /// Returns an error when the chain or the key cannot be generated.
    pub fn ephemeral(common_name: &str) -> Result<Self, Error> {
        let signer = EphemeralSigner::new(common_name)?;
        Ok(Self {
            signer: Box::new(signer),
            kind: Kind::Ephemeral,
            time_authority: None,
        })
    }

    /// A chain and its key, both in PEM, for the given algorithm.
    ///
    /// The RSA algorithms are refused: the RSA implementation the provenance library links leaks timing
    /// from its private-key operations, and no fixed release exists. A key this product signs with is an
    /// elliptic-curve or an Edwards-curve key until that changes.
    ///
    /// # Errors
    ///
    /// Returns an error when the algorithm is one of the RSA ones, when the chain or the key does not
    /// parse, or when the key is not for the algorithm.
    pub fn held(chain_pem: &[u8], key_pem: &[u8], algorithm: Algorithm) -> Result<Self, Error> {
        if matches!(algorithm, Algorithm::Ps256 | Algorithm::Ps384 | Algorithm::Ps512) {
            return Err(Error::Credential {
                reason:
                    "RSA keys are not signed with, because the RSA implementation available leaks timing \
                         from private-key operations; use an ES256, ES384, ES512 or Ed25519 key"
                        .to_owned(),
            });
        }
        let signer = c2pa::create_signer::from_keys(chain_pem, key_pem, algorithm.library(), None).map_err(
            |cause| Error::Credential {
                reason: cause.to_string(),
            },
        )?;
        Ok(Self {
            signer,
            kind: Kind::Held,
            time_authority: None,
        })
    }

    /// The same credential, asking this authority for a time stamp at every signing.
    #[must_use]
    pub fn stamped_by(mut self, authority: &str) -> Self {
        self.time_authority = Some(authority.to_owned());
        self
    }

    /// Which kind of credential this is.
    #[must_use]
    pub const fn kind(&self) -> Kind {
        self.kind
    }

    /// The authority asked for a time stamp, if one is configured.
    #[must_use]
    pub fn time_authority(&self) -> Option<&str> {
        self.time_authority.as_deref()
    }
}

impl Signer for Credential {
    fn sign(&self, data: &[u8]) -> c2pa::Result<Vec<u8>> {
        self.signer.sign(data)
    }

    fn alg(&self) -> SigningAlg {
        self.signer.alg()
    }

    fn certs(&self) -> c2pa::Result<Vec<Vec<u8>>> {
        self.signer.certs()
    }

    fn reserve_size(&self) -> usize {
        // A time-stamp token rides inside the signature, and its size is the authority's to decide; the
        // reservation leaves room for a large one so that a signing never fails on that account.
        self.signer.reserve_size().saturating_add(16 * 1024)
    }

    fn time_authority_url(&self) -> Option<String> {
        self.time_authority.clone()
    }
}
