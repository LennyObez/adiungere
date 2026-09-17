//! Signing: the facts adiungere established about a file, carried in a Content Credentials manifest
//! signed last, after every byte of the asset is final.
//!
//! An output of the product is signed with the manifest **embedded**: the store goes into the container
//! as the box the standard reserves for it, with the hard binding over the rest of the file. An original
//! recording is never touched, so its manifest is written **beside** it, as a sidecar the validators look
//! for under the asset's stem. Either way the manifest carries three things: the recordings the asset came
//! from as ingredients, each with the digest of its bytes, the parent first; the actions, `c2pa.opened`
//! over those ingredients and then the one the export class corresponds to; and the adiungere manifest of
//! the asset as the `com.adiungere.integrity` assertion, so that every fingerprint this product computed
//! is inside the signed claim and a validator that knows nothing of adiungere still shows it.

use std::fs::File;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use adiungere_manifest::{ExportClass, Manifest, Operation};
use c2pa::assertions::Action;
use c2pa::{Builder, Context, Signer};

use crate::error::Error;

/// The assertion label under which the adiungere manifest travels inside a Content Credentials manifest.
pub const INTEGRITY_ASSERTION: &str = "com.adiungere.integrity";

/// The name the manifest names as its generator.
pub const GENERATOR: &str = "adiungere";

/// Where a manifest is placed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Placement {
    /// Inside the asset, with the hard binding over the asset's own bytes.
    Embedded,
    /// Beside the asset, under its stem with the `c2pa` extension, the asset untouched.
    Sidecar,
}

/// What to sign: the asset, the recordings it was made from, and the facts to carry.
#[derive(Debug)]
pub struct Request<'a> {
    /// The file the manifest is about.
    pub asset: &'a Path,
    /// The recordings the asset was made from, in the order the facts list them; the first is the parent.
    /// Empty for an original, whose only ingredient is itself.
    pub sources: Vec<&'a Path>,
    /// The adiungere manifest of the asset, carried whole as the integrity assertion.
    pub facts: &'a Manifest,
    /// Where the manifest goes.
    pub placement: Placement,
}

/// What a signing produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signed {
    /// Where the manifest went.
    pub placement: Placement,
    /// The file written: the signed asset, or the sidecar.
    pub written: PathBuf,
    /// The manifest store, as embedded or as written beside the asset.
    pub store: Vec<u8>,
    /// Whether a time-stamping authority was asked and answered; the token is inside the store.
    pub time_stamped: bool,
}

/// The sidecar path the validators look for: the asset's stem with the `c2pa` extension.
#[must_use]
pub fn sidecar_beside(asset: &Path) -> PathBuf {
    asset.with_extension("c2pa")
}

/// The media type the asset is signed as, from its extension.
fn format_of(asset: &Path) -> String {
    c2pa::format_from_path(asset).unwrap_or_else(|| "video/mp4".to_owned())
}

fn file_name(path: &Path) -> String {
    path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn open(path: &Path) -> Result<File, Error> {
    File::open(path).map_err(|cause| Error::Io {
        path: path.to_path_buf(),
        cause,
    })
}

/// Whether two paths name one file, whatever their spelling, with a file not yet on disk resolved through
/// its directory.
fn same_file(existing: &Path, written: &Path) -> bool {
    let Ok(existing) = std::fs::canonicalize(existing) else {
        return false;
    };
    let resolved = std::fs::canonicalize(written).or_else(|_| {
        let parent = written.parent().filter(|parent| !parent.as_os_str().is_empty());
        let directory = std::fs::canonicalize(parent.unwrap_or(Path::new(".")))?;
        written
            .file_name()
            .map(|name| directory.join(name))
            .ok_or_else(|| std::io::Error::other("no file name"))
    });
    resolved.is_ok_and(|resolved| resolved == existing)
}

/// The action the export class corresponds to, after `c2pa.opened`; none for an original.
fn action_of(facts: &Manifest) -> Option<&'static str> {
    match &facts.produced.operation {
        Operation::Inspection { .. } => None,
        Operation::Export { class, .. } => Some(match class {
            ExportClass::Extraction | ExportClass::TwoTrackArchive => "c2pa.repackaged",
            ExportClass::RenditionLossy | ExportClass::RenditionLosslessVerified => "c2pa.transcoded",
        }),
    }
}

/// The builder with the definition, the ingredients and the actions of a request, before signing.
fn builder_for(request: &Request<'_>) -> Result<Builder, Error> {
    let facts = serde_json::to_value(request.facts).map_err(|cause| Error::Facts { cause })?;
    let definition = serde_json::json!({
        "title": file_name(request.asset),
        "claim_generator_info": [{"name": GENERATOR, "version": request.facts.produced.version}],
        "assertions": [{"label": INTEGRITY_ASSERTION, "data": facts}],
    });
    let mut builder = Builder::from_context(Context::new()).with_definition(definition.to_string())?;

    // An original has no source but itself: what the manifest says is that adiungere opened these bytes
    // and established the facts about them, and the ingredient's digest is the digest of the file.
    let sources: Vec<&Path> = if request.sources.is_empty() {
        vec![request.asset]
    } else {
        request.sources.clone()
    };
    let mut labels = Vec::new();
    for (position, source) in sources.iter().enumerate() {
        let label = format!("source-{position}");
        let relationship = if position == 0 { "parentOf" } else { "componentOf" };
        let ingredient = serde_json::json!({
            "title": file_name(source),
            "relationship": relationship,
            "label": label,
        });
        builder.add_ingredient_from_stream(ingredient.to_string(), &format_of(source), &mut open(source)?)?;
        labels.push(label);
    }
    builder.add_action(Action::new("c2pa.opened").set_parameter("ingredientIds", labels)?)?;
    if let Some(action) = action_of(request.facts) {
        builder.add_action(Action::new(action))?;
    }
    Ok(builder)
}

/// Signs the asset as the request says. For an embedded manifest `out` is the signed copy of the asset,
/// written under a temporary name and moved into place once complete; for a sidecar `out` is the manifest
/// file, beside the asset by default. Nothing this writes may name the asset or a source.
///
/// # Errors
///
/// Returns an error when a path to write names a file being read, when a file cannot be read or written,
/// when the facts cannot be carried, when the library refuses, or when the signer asks an authority for
/// a time stamp and gets none.
pub fn sign(request: &Request<'_>, signer: &dyn Signer, out: Option<&Path>) -> Result<Signed, Error> {
    let format = format_of(request.asset);
    let written = out.map_or_else(
        || match request.placement {
            Placement::Embedded => request.asset.with_extension("signed.mp4"),
            Placement::Sidecar => sidecar_beside(request.asset),
        },
        Path::to_path_buf,
    );
    let partial = written.with_extension("part");
    let read: Vec<&Path> = std::iter::once(request.asset)
        .chain(request.sources.iter().copied())
        .collect();
    for path in [&written, &partial] {
        if read.iter().any(|source| same_file(source, path)) {
            return Err(Error::Overwrite { path: path.clone() });
        }
    }

    let mut builder = builder_for(request)?;
    let mut source = open(request.asset)?;
    let store = match request.placement {
        Placement::Embedded => {
            let mut dest = File::options()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&partial)
                .map_err(|cause| Error::Io {
                    path: partial.clone(),
                    cause,
                })?;
            let outcome = builder.sign(signer, &format, &mut source, &mut dest);
            drop(dest);
            let store = match outcome {
                Ok(store) => store,
                Err(cause) => {
                    let _ = std::fs::remove_file(&partial);
                    return Err(cause.into());
                },
            };
            std::fs::rename(&partial, &written).map_err(|cause| Error::Io {
                path: written.clone(),
                cause,
            })?;
            store
        },
        Placement::Sidecar => {
            builder.set_no_embed(true);
            let mut discarded = Cursor::new(Vec::new());
            let store = builder.sign(signer, &format, &mut source, &mut discarded)?;
            std::fs::write(&written, &store).map_err(|cause| Error::Io {
                path: written.clone(),
                cause,
            })?;
            store
        },
    };

    Ok(Signed {
        placement: request.placement,
        written,
        store,
        time_stamped: signer.time_authority_url().is_some(),
    })
}
