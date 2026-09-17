//! The manifest: what was seen, as facts, and the words the product may use about them.
//!
//! Three things live here. The [`Manifest`] types, version 1, with unknown fields refused and a published
//! schema. The comparison of a manifest with a file, which produces findings per subject and never a
//! verdict. And the wording catalogue, which holds every sentence the product forms and is checked against
//! a list of words that read as verdicts.

#![forbid(unsafe_code)]

mod build;
mod manifest;
pub mod time;
mod verify;
pub mod wording;

pub use adiungere_fingerprint::{AnnexBDigest, Digest, StructuralFingerprint, TrackFingerprint};
pub use build::{Producer, build, build_for_export};
pub use manifest::{
    EditEntry, ExportClass, FileRecord, MANIFEST_FORMAT, Manifest, Masking, Operation, Produced, Scope,
    SourceRecord, SourceTrack, Structure, TrackKind, TrackRecord, VendorBox, VendorLocation, VendorRecord,
};
pub use verify::{Finding, Outcome, Subject, Verification, verify};

use std::fmt;

/// Why a manifest could not be built or compared.
#[derive(Debug)]
pub enum Error {
    /// The container could not be read.
    Container(adiungere_isobmff::Error),
    /// A fingerprint could not be computed.
    Fingerprint(adiungere_fingerprint::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Container(cause) => write!(formatter, "{cause}"),
            Self::Fingerprint(cause) => write!(formatter, "{cause}"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Container(cause) => Some(cause),
            Self::Fingerprint(cause) => Some(cause),
        }
    }
}

impl From<adiungere_isobmff::Error> for Error {
    fn from(cause: adiungere_isobmff::Error) -> Self {
        Self::Container(cause)
    }
}

impl From<adiungere_fingerprint::Error> for Error {
    fn from(cause: adiungere_fingerprint::Error) -> Self {
        Self::Fingerprint(cause)
    }
}

/// The JSON schema of the manifest, as the types derive it.
///
/// # Errors
///
/// Returns an error only if serialisation fails, which the schema type does not allow.
pub fn schema() -> Result<String, serde_json::Error> {
    let generator = schemars::generate::SchemaSettings::draft2020_12().into_generator();
    let schema = generator.into_root_schema_for::<Manifest>();
    let mut text = serde_json::to_string_pretty(&with_keys_sorted(serde_json::to_value(schema)?))?;
    text.push('\n');
    Ok(text)
}

/// The same value with every object's keys in sorted order, however the JSON library orders them: the
/// published schema is compared byte for byte, and that comparison must not depend on which crates were
/// compiled alongside this one.
fn with_keys_sorted(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => {
            let mut entries: Vec<(String, serde_json::Value)> = map.into_iter().collect();
            entries.sort_by(|(a, _), (b, _)| a.cmp(b));
            let mut sorted = serde_json::Map::new();
            for (key, inner) in entries {
                sorted.insert(key, with_keys_sorted(inner));
            }
            serde_json::Value::Object(sorted)
        },
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(with_keys_sorted).collect())
        },
        other => other,
    }
}
