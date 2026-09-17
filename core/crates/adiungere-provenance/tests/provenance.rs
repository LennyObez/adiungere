//! Signing and reading back on the synthetic corpus: an export signed last with the manifest embedded, an
//! original with a sidecar and not one byte changed, a rewrite after signing seen to invalidate, and a
//! signature made through a delegate that never sees the media.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use adiungere_fixtures::{Spec, build};
use adiungere_isobmff::{Input, RemuxPlan, SliceSource, parse, remux};
use adiungere_manifest::{ExportClass, Manifest, Masking, Operation, Producer, Scope};
use adiungere_provenance::{
    Algorithm, Credential, Delegated, Error, Placement, Request, State, check, read, sidecar_beside, sign,
};
use c2pa::Signer as _;

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-provenance-{name}-{}-{:?}",
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

fn producer() -> Producer {
    Producer::current("adiungere", "0.0.0")
}

/// The reference-like recording written to a directory, with its inspection manifest.
fn original_in(directory: &Path) -> Result<(PathBuf, Manifest), Box<dyn std::error::Error>> {
    let path = directory.join("20260604_122323E.MP4");
    build(&Spec::reference_like())?.write_to(&path)?;
    let mut source = adiungere_isobmff::FileSource::open(&path)?;
    let facts = adiungere_manifest::build(
        &mut source,
        "20260604_122323E.MP4",
        Scope::Full,
        &producer(),
        None,
    )?;
    Ok((path, facts))
}

/// A rear-only extraction of the recording, written to a directory, with its export manifest.
fn export_in(directory: &Path, original: &[u8]) -> Result<(PathBuf, Manifest), Box<dyn std::error::Error>> {
    let mut source = SliceSource::new(original);
    let container = parse(&mut source)?;
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![1, 2],
    }];
    let mut out = Vec::new();
    remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true)?;
    let path = directory.join("rear.mp4");
    std::fs::write(&path, &out)?;
    let mut written = SliceSource::new(&out);
    let facts = adiungere_manifest::build_for_export(
        &mut written,
        "rear.mp4",
        &producer(),
        None,
        Operation::Export {
            class: ExportClass::Extraction,
            masking: Masking::None,
            sources: Vec::new(),
        },
    )?;
    Ok((path, facts))
}

/// A rewrite that keeps every sample and the manifest store, and moves the boxes: what a rewriter that
/// does not know the standard produces.
fn rewrite(bytes: &[u8]) -> Result<Vec<u8>, adiungere_isobmff::Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let all: Vec<usize> = (0..container.tracks()?.len()).collect();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: all,
    }];
    let plan = RemuxPlan {
        keep_manifest_store: true,
        ..RemuxPlan::default()
    };
    let mut out = Vec::new();
    remux(&mut inputs, &plan, &mut out, &mut |_| true)?;
    Ok(out)
}

#[test]
fn an_export_is_signed_with_the_manifest_embedded_and_reads_back_with_its_facts() {
    // Arrange
    let directory = Temporary::new("embedded");
    let (original, _) = original_in(&directory.0).unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    let (asset, facts) = export_in(&directory.0, &original_bytes).unwrap();
    let credential = Credential::ephemeral("adiungere.local").unwrap();
    let out = directory.0.join("rear.signed.mp4");

    // Act
    let signed = sign(
        &Request {
            asset: &asset,
            sources: vec![&original],
            facts: &facts,
            placement: Placement::Embedded,
        },
        &credential,
        Some(&out),
    )
    .unwrap();
    let credentials = read(&out, None).unwrap();

    // Assert
    assert_eq!(signed.placement, Placement::Embedded);
    assert_eq!(signed.written, out);
    assert!(!signed.time_stamped);
    assert!(signed.store.len() > 1000);
    assert_eq!(credentials.placement, Placement::Embedded);
    assert_eq!(credentials.state, State::ValidNotOnTrustList);
    assert_eq!(credentials.actions, vec!["c2pa.opened", "c2pa.repackaged"]);
    assert_eq!(credentials.ingredients.len(), 1);
    assert_eq!(
        credentials
            .ingredients
            .first()
            .map(|i| (i.title.as_deref(), i.relationship.as_str())),
        Some((Some("20260604_122323E.MP4"), "parentOf"))
    );
    assert_eq!(credentials.integrity.as_ref(), Some(&facts));
    assert!(
        credentials
            .integrity
            .as_ref()
            .is_some_and(|facts| facts.file.sha256.is_some()),
        "the signed facts carry the asset's digest"
    );
    assert_eq!(credentials.generator.as_deref(), Some("adiungere 0.0.0"));
    assert_eq!(
        credentials.signer.as_ref().and_then(|s| s.common_name.as_deref()),
        Some("adiungere.local")
    );
    assert!(credentials.time_stamped_at.is_none());
    assert!(
        credentials
            .statuses
            .iter()
            .any(|s| s.code == "signingCredential.untrusted" && !s.passed),
        "{:?}",
        credentials.statuses
    );
    assert!(std::fs::read(&asset).unwrap().len() < std::fs::read(&out).unwrap().len());
    assert!(!directory.0.join("rear.signed.mp4.signing").exists());
}

#[test]
fn an_original_gets_a_sidecar_and_not_one_byte_of_it_changes() {
    // Arrange
    let directory = Temporary::new("sidecar");
    let (original, facts) = original_in(&directory.0).unwrap();
    let bytes_before = std::fs::read(&original).unwrap();
    let modified_before = std::fs::metadata(&original).unwrap().modified().unwrap();
    let credential = Credential::ephemeral("adiungere.local").unwrap();

    // Act
    let signed = sign(
        &Request {
            asset: &original,
            sources: Vec::new(),
            facts: &facts,
            placement: Placement::Sidecar,
        },
        &credential,
        None,
    )
    .unwrap();
    let credentials = read(&original, None).unwrap();

    // Assert
    assert_eq!(signed.written, sidecar_beside(&original));
    assert_eq!(signed.written.extension().and_then(|e| e.to_str()), Some("c2pa"));
    assert_eq!(std::fs::read(&signed.written).unwrap(), signed.store);
    assert_eq!(std::fs::read(&original).unwrap(), bytes_before);
    assert_eq!(
        std::fs::metadata(&original).unwrap().modified().unwrap(),
        modified_before
    );
    assert_eq!(credentials.placement, Placement::Sidecar);
    assert_eq!(credentials.read_from, signed.written);
    assert_eq!(credentials.state, State::ValidNotOnTrustList);
    assert_eq!(credentials.actions, vec!["c2pa.opened"]);
    assert_eq!(
        credentials
            .ingredients
            .first()
            .map(|i| (i.title.as_deref(), i.relationship.as_str())),
        Some((Some("20260604_122323E.MP4"), "parentOf"))
    );
    assert_eq!(credentials.integrity.as_ref(), Some(&facts));
}

#[test]
fn a_rewrite_after_signing_is_reported_as_a_broken_binding() {
    // Signing is the last step: a rewrite that keeps every sample and every box, and only moves them,
    // breaks the hard binding, and the validator says so.

    // Arrange
    let directory = Temporary::new("rewrite");
    let (original, _) = original_in(&directory.0).unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    let (asset, facts) = export_in(&directory.0, &original_bytes).unwrap();
    let credential = Credential::ephemeral("adiungere.local").unwrap();
    let signed_path = directory.0.join("rear.signed.mp4");
    sign(
        &Request {
            asset: &asset,
            sources: vec![&original],
            facts: &facts,
            placement: Placement::Embedded,
        },
        &credential,
        Some(&signed_path),
    )
    .unwrap();
    let signed_bytes = std::fs::read(&signed_path).unwrap();
    let rewritten_path = directory.0.join("rewritten.mp4");
    std::fs::write(&rewritten_path, rewrite(&signed_bytes).unwrap()).unwrap();

    // Act
    let intact = read(&signed_path, None).unwrap();
    let broken = read(&rewritten_path, None).unwrap();

    // Assert
    assert_eq!(intact.state, State::ValidNotOnTrustList);
    assert_eq!(broken.state, State::Invalid);
    assert!(
        broken
            .statuses
            .iter()
            .any(|s| s.code == "assertion.bmffHash.mismatch" && !s.passed),
        "{:?}",
        broken.statuses
    );
}

#[test]
fn signing_refuses_to_write_where_it_reads() {
    // Arrange
    let directory = Temporary::new("overwrite");
    let (original, facts) = original_in(&directory.0).unwrap();
    let bytes_before = std::fs::read(&original).unwrap();
    let credential = Credential::ephemeral("adiungere.local").unwrap();
    let same_by_another_route = directory
        .0
        .join("..")
        .join(directory.0.file_name().unwrap())
        .join("20260604_122323E.MP4");

    // Act
    let as_output = sign(
        &Request {
            asset: &original,
            sources: Vec::new(),
            facts: &facts,
            placement: Placement::Embedded,
        },
        &credential,
        Some(&same_by_another_route),
    );
    let as_sidecar = sign(
        &Request {
            asset: &original,
            sources: Vec::new(),
            facts: &facts,
            placement: Placement::Sidecar,
        },
        &credential,
        Some(&same_by_another_route),
    );

    // Assert
    assert!(matches!(as_output, Err(Error::Overwrite { .. })), "{as_output:?}");
    assert!(
        matches!(as_sidecar, Err(Error::Overwrite { .. })),
        "{as_sidecar:?}"
    );
    assert_eq!(std::fs::read(&original).unwrap(), bytes_before);
}

#[test]
fn a_file_without_credentials_is_said_to_have_none() {
    // Arrange
    let directory = Temporary::new("none");
    let (original, _) = original_in(&directory.0).unwrap();

    // Act
    let outcome = read(&original, None);

    // Assert
    assert!(matches!(outcome, Err(Error::NoCredentials { .. })), "{outcome:?}");
}

#[test]
fn a_delegate_signs_a_few_kilobytes_that_carry_none_of_the_media() {
    // The split flow: the manifest is built here, the signature is made by a function given only the
    // bytes to sign, which name the assertions by digest and carry no sample.

    // Arrange
    let directory = Temporary::new("delegated");
    let (original, _) = original_in(&directory.0).unwrap();
    let original_bytes = std::fs::read(&original).unwrap();
    let (asset, facts) = export_in(&directory.0, &original_bytes).unwrap();
    let asset_bytes = std::fs::read(&asset).unwrap();
    let key_holder = Credential::ephemeral("relay.adiungere.local").unwrap();
    let chain = key_holder.certs().unwrap();
    let handed_over: Mutex<Vec<Vec<u8>>> = Mutex::new(Vec::new());
    let delegated = Delegated::new(chain, Algorithm::Ed25519, key_holder.reserve_size(), |bytes| {
        handed_over
            .lock()
            .map_err(|_| "poisoned".to_owned())?
            .push(bytes.to_vec());
        key_holder.sign(bytes).map_err(|e| e.to_string())
    });
    let out = directory.0.join("rear.signed.mp4");

    // Act
    sign(
        &Request {
            asset: &asset,
            sources: vec![&original],
            facts: &facts,
            placement: Placement::Embedded,
        },
        &delegated,
        Some(&out),
    )
    .unwrap();
    let credentials = read(&out, None).unwrap();

    // Assert
    let handed = handed_over.into_inner().unwrap();
    assert_eq!(handed.len(), 1, "one signature was asked for");
    let to_be_signed = handed.first().unwrap();
    assert!(to_be_signed.len() < 16 * 1024, "{} bytes", to_be_signed.len());
    let a_sample = asset_bytes.get(asset_bytes.len() - 64..).unwrap();
    assert!(
        !to_be_signed.windows(64).any(|window| window == a_sample),
        "the bytes to sign carry media"
    );
    assert_eq!(credentials.state, State::ValidNotOnTrustList);
    assert_eq!(
        credentials.signer.as_ref().and_then(|s| s.common_name.as_deref()),
        Some("relay.adiungere.local")
    );
}

#[test]
fn a_token_from_an_authority_names_the_bytes_it_was_asked_about_and_no_others() {
    // Arrange: a token obtained once from a public authority over the reference-like recording.
    let bytes = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let token = std::fs::read(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/reference-like.tsr"
    ))
    .unwrap();
    let mut other = bytes.clone();
    if let Some(byte) = other.get_mut(100) {
        *byte ^= 1;
    }

    // Act
    let attested = check(&token, &bytes).unwrap();
    let refused = check(&token, &other);

    // Assert
    assert_eq!(attested.time, "20260917130016Z");
    assert!(!attested.trust_checked);
    assert!(matches!(refused, Err(Error::TokenMismatch { .. })), "{refused:?}");
}
