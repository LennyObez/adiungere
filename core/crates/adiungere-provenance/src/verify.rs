//! Reading credentials back, with the trust state the validator found and nothing rounded up.
//!
//! A manifest is either embedded in the asset or beside it. Its validation has three outcomes and this
//! crate keeps all three apart: the signature and the hard binding hold and the signer's chain leads to
//! the trust list; they hold but the chain leads to no trust list, which is what a self-signed credential
//! and a certificate from an unlisted authority both give; or something does not hold, and the codes say
//! what. A reader is never shown the first when the validator found the second.

use std::fs::File;
use std::path::{Path, PathBuf};

use adiungere_manifest::Manifest;
use c2pa::{Context, Reader, ValidationState};

use crate::error::Error;
use crate::sign::{INTEGRITY_ASSERTION, Placement, sidecar_beside};

/// What the validation of a manifest found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    /// The signature and the hard binding hold, and the signer's chain leads to the trust list.
    Trusted,
    /// The signature and the hard binding hold; the signer's chain leads to no trust list.
    ValidNotOnTrustList,
    /// The signature or the hard binding does not hold; the codes say what.
    Invalid,
}

/// The signer as the certificate names it.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SignerIdentity {
    /// The certificate's common name.
    pub common_name: Option<String>,
    /// The issuer's name.
    pub issuer: Option<String>,
    /// The signature algorithm.
    pub algorithm: Option<String>,
    /// The certificate's serial number.
    pub serial: Option<String>,
}

/// One code the validator reported.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Status {
    /// The code, as the standard names it.
    pub code: String,
    /// Whether the check it names passed.
    pub passed: bool,
    /// The validator's explanation.
    pub explanation: Option<String>,
}

/// One ingredient the manifest names.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct IngredientSummary {
    /// The title the manifest gave it.
    pub title: Option<String>,
    /// Its relationship to the asset.
    pub relationship: String,
    /// The digest of its bytes, when the manifest's ingredient version records one; the current version
    /// does not, and the digest of every source is in the integrity assertion instead.
    pub hash: Option<String>,
}

/// Everything read from a manifest, with the trust state the validator found.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Credentials {
    /// Where the manifest was found.
    pub placement: Placement,
    /// The file the manifest was read from: the asset, or the sidecar.
    pub read_from: PathBuf,
    /// What the validation found.
    pub state: State,
    /// The signer, when the manifest carries a signature that could be read.
    pub signer: Option<SignerIdentity>,
    /// The time the time-stamping authority attested, when a token is present and valid.
    pub time_stamped_at: Option<String>,
    /// The manifest's generator, as it named itself.
    pub generator: Option<String>,
    /// The actions the manifest records, in order.
    pub actions: Vec<String>,
    /// The ingredients the manifest names.
    pub ingredients: Vec<IngredientSummary>,
    /// The adiungere manifest the integrity assertion carries, when one is there and reads.
    pub integrity: Option<Manifest>,
    /// Every code the validator reported, failures first.
    pub statuses: Vec<Status>,
}

/// Reads the credentials of an asset: the embedded manifest when there is one, otherwise the sidecar
/// given or the one beside the asset. The asset is opened for reading only.
///
/// # Errors
///
/// Returns an error when the asset or the sidecar cannot be read, when the library refuses, or when
/// neither an embedded manifest nor a sidecar exists.
pub fn read(asset: &Path, sidecar: Option<&Path>) -> Result<Credentials, Error> {
    let format = crate::sign::format_of(asset);
    let stream = File::open(asset).map_err(|cause| Error::Io {
        path: asset.to_path_buf(),
        cause,
    })?;

    let embedded = Reader::from_context(Context::new()).with_stream(&format, stream);
    let (reader, placement, read_from) = match embedded {
        Ok(reader) => (reader, Placement::Embedded, asset.to_path_buf()),
        Err(c2pa::Error::JumbfNotFound) => {
            let path = sidecar.map_or_else(|| sidecar_beside(asset), Path::to_path_buf);
            if !path.is_file() {
                return Err(Error::NoCredentials {
                    path: asset.to_path_buf(),
                });
            }
            let store = std::fs::read(&path).map_err(|cause| Error::Io {
                path: path.clone(),
                cause,
            })?;
            let stream = File::open(asset).map_err(|cause| Error::Io {
                path: asset.to_path_buf(),
                cause,
            })?;
            let reader = Reader::from_context(Context::new())
                .with_manifest_data_and_stream(&store, &format, stream)?;
            (reader, Placement::Sidecar, path)
        },
        Err(cause) => return Err(cause.into()),
    };

    Ok(summarise(&reader, placement, read_from))
}

/// What the reader found, as this crate reports it.
fn summarise(reader: &Reader, placement: Placement, read_from: PathBuf) -> Credentials {
    let state = match reader.validation_state() {
        ValidationState::Trusted => State::Trusted,
        ValidationState::Valid => State::ValidNotOnTrustList,
        ValidationState::Invalid => State::Invalid,
    };
    let mut statuses: Vec<Status> = reader
        .validation_status()
        .unwrap_or(&[])
        .iter()
        .map(|status| Status {
            code: status.code().to_owned(),
            passed: status.passed(),
            explanation: status.explanation().map(str::to_owned),
        })
        .collect();
    statuses.sort_by_key(|status| status.passed);

    let active = reader.active_manifest();
    let signer = active
        .and_then(c2pa::Manifest::signature_info)
        .map(|info| SignerIdentity {
            common_name: info.common_name.clone(),
            issuer: info.issuer.clone(),
            algorithm: info.alg.map(|alg| alg.to_string()),
            serial: info.cert_serial_number.clone(),
        });
    let time_stamped_at = active
        .and_then(c2pa::Manifest::signature_info)
        .and_then(|info| info.time.clone());
    let generator = active.and_then(|manifest| {
        manifest
            .claim_generator_info
            .as_ref()
            .and_then(|generators| generators.first())
            .map(|generator| match &generator.version {
                Some(version) => format!("{} {version}", generator.name),
                None => generator.name.clone(),
            })
    });
    let actions = active.map_or_else(Vec::new, actions_of);
    let ingredients = active.map_or_else(Vec::new, |manifest| {
        manifest
            .ingredients()
            .iter()
            .map(|ingredient| IngredientSummary {
                title: ingredient.title().map(str::to_owned),
                relationship: ingredient.relationship().as_str().to_owned(),
                hash: ingredient.hash().map(str::to_owned),
            })
            .collect()
    });
    let integrity = active.and_then(|manifest| {
        manifest
            .assertions()
            .iter()
            .find(|assertion| assertion.label() == INTEGRITY_ASSERTION)
            .and_then(|assertion| assertion.value().ok())
            .and_then(|value| serde_json::from_value::<Manifest>(value.clone()).ok())
    });

    Credentials {
        placement,
        read_from,
        state,
        signer,
        time_stamped_at,
        generator,
        actions,
        ingredients,
        integrity,
        statuses,
    }
}

/// The actions of a manifest, in the order its actions assertion lists them.
fn actions_of(manifest: &c2pa::Manifest) -> Vec<String> {
    manifest
        .assertions()
        .iter()
        .filter(|assertion| assertion.label().starts_with("c2pa.actions"))
        .filter_map(|assertion| assertion.value().ok())
        .flat_map(|value| {
            value
                .get("actions")
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .filter_map(|action| {
            action
                .get("action")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .collect()
}
