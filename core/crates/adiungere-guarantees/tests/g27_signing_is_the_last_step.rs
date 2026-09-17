//! **G27.** Signing is the last step: a rewrite applied after signing invalidates the binding, an
//! independent validator says so, and re-signing restores it.
//!
//! The hard binding covers the file's bytes, so anything that moves a box after the signature, a
//! faststart included, is a change the validator reports. The product therefore signs last and never
//! rewrites what it signed. This guarantee holds three things with the pinned validator, which is not
//! this product: the export the product signs is valid; the same file remuxed, every sample kept and
//! only the boxes moved, is invalid on the hash of the container; and signing the remuxed file again,
//! with the recording as its source, makes it valid again.

use std::path::{Path, PathBuf};

use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::{pinned_validator, validator_verdict};
use adiungere_isobmff::{Input, RemuxPlan, SliceSource, parse, remux};
use adiungere_manifest::{ExportClass, Manifest, Masking, Operation, Producer};
use adiungere_provenance::{Credential, Placement, Request, sign};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g27-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A remux of every track of a file, written beside it under another name.
fn remuxed(from: &Path, to: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let bytes = std::fs::read(from)?;
    let mut source = SliceSource::new(&bytes);
    let container = parse(&mut source)?;
    let all: Vec<usize> = (0..container.tracks()?.len()).collect();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: all,
    }];
    let mut out = Vec::new();
    remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true)?;
    std::fs::write(to, out)?;
    Ok(())
}

/// The export manifest of a file written by the product, with the recording as its one source.
fn export_facts(path: &Path) -> Result<Manifest, Box<dyn std::error::Error>> {
    let mut source = adiungere_isobmff::FileSource::open(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(adiungere_manifest::build_for_export(
        &mut source,
        &name,
        &Producer::current("adiungere", "0.0.0"),
        None,
        Operation::Export {
            class: ExportClass::Extraction,
            masking: Masking::None,
            sources: Vec::new(),
        },
    )?)
}

/// The rear-only extraction of the reference-like recording, unsigned, beside the recording.
fn recording_and_export(directory: &Path) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let recording = directory.join("20260604_122323E.MP4");
    build(&Spec::reference_like())?.write_to(&recording)?;
    let bytes = std::fs::read(&recording)?;
    let mut source = SliceSource::new(&bytes);
    let container = parse(&mut source)?;
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![1, 2],
    }];
    let mut out = Vec::new();
    remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true)?;
    let export = directory.join("rear.mp4");
    std::fs::write(&export, out)?;
    Ok((recording, export))
}

fn sign_embedded(asset: &Path, recording: &Path, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let facts = export_facts(asset)?;
    let credential = Credential::ephemeral("adiungere.local")?;
    sign(
        &Request {
            asset,
            sources: vec![recording],
            facts: &facts,
            placement: Placement::Embedded,
        },
        &credential,
        Some(out),
    )?;
    Ok(())
}

#[test]
fn a_rewrite_after_signing_is_invalid_to_the_validator_and_signing_again_restores_it() {
    // Arrange
    let validator = pinned_validator().unwrap();
    let directory = Temporary::new("order");
    let (recording, export) = recording_and_export(&directory.0).unwrap();
    let signed = directory.0.join("rear.signed.mp4");
    sign_embedded(&export, &recording, &signed).unwrap();
    let rewritten = directory.0.join("rear.rewritten.mp4");
    remuxed(&signed, &rewritten).unwrap();
    let resigned = directory.0.join("rear.resigned.mp4");
    sign_embedded(&rewritten, &recording, &resigned).unwrap();

    // Act
    let intact = validator_verdict(&validator, &signed).unwrap();
    let broken = validator_verdict(&validator, &rewritten).unwrap();
    let restored = validator_verdict(&validator, &resigned).unwrap();

    // Assert
    assert_eq!(intact.state, "Valid", "{:?}", intact.failures);
    assert_eq!(intact.failures, vec!["signingCredential.untrusted"]);
    assert_eq!(broken.state, "Invalid");
    assert!(
        broken
            .failures
            .iter()
            .any(|code| code == "assertion.bmffHash.mismatch"),
        "{:?}",
        broken.failures
    );
    assert_eq!(restored.state, "Valid", "{:?}", restored.failures);
    assert_eq!(restored.failures, vec!["signingCredential.untrusted"]);
}

#[test]
fn the_product_reports_the_same_order_as_the_validator() {
    // The product's own reader agrees with the validator on the intact file and on the rewritten one,
    // so what a person sees from the product is what a stranger sees from the validator.

    // Arrange
    let directory = Temporary::new("agreement");
    let (recording, export) = recording_and_export(&directory.0).unwrap();
    let signed = directory.0.join("rear.signed.mp4");
    sign_embedded(&export, &recording, &signed).unwrap();
    let rewritten = directory.0.join("rear.rewritten.mp4");
    remuxed(&signed, &rewritten).unwrap();

    // Act
    let intact = adiungere_provenance::read(&signed, None).unwrap();
    let broken = adiungere_provenance::read(&rewritten, None).unwrap();

    // Assert
    assert_eq!(intact.state, adiungere_provenance::State::ValidNotOnTrustList);
    assert_eq!(broken.state, adiungere_provenance::State::Invalid);
    assert!(
        broken
            .statuses
            .iter()
            .any(|status| status.code == "assertion.bmffHash.mismatch" && !status.passed)
    );
}
