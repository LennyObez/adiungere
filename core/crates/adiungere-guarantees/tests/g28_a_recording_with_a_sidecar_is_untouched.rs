//! **G28.** A recording that receives a sidecar manifest is byte for byte, and to the second, the
//! recording it was, and the pinned validator accepts the pair.
//!
//! The product never writes into a recording: that is what makes its facts about one worth anything.
//! Content Credentials for a recording therefore live beside it, under its stem, where the validators
//! look. Every corpus recording gets a sidecar here; its bytes and its modification time are compared
//! before and after, and the validator reads the pair and finds the binding intact. A detection test
//! shows the same validator refusing the pair once one byte of the recording has changed.

use std::path::{Path, PathBuf};

use adiungere_fingerprint::Digest;
use adiungere_fixtures::{build, corpus};
use adiungere_guarantees::{pinned_validator, validator_verdict};
use adiungere_manifest::{Manifest, Producer, Scope};
use adiungere_provenance::{Credential, Placement, Request, sign};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g28-{name}-{}-{:?}",
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

fn facts_of(path: &Path) -> Result<Manifest, Box<dyn std::error::Error>> {
    let mut source = adiungere_isobmff::FileSource::open(path)?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    Ok(adiungere_manifest::build(
        &mut source,
        &name,
        Scope::Full,
        &Producer::current("adiungere", "0.0.0"),
        None,
    )?)
}

/// The digest of a file's bytes and its modification time.
fn state_of(path: &Path) -> (String, std::time::SystemTime) {
    let bytes = std::fs::read(path).unwrap_or_default();
    let modified = std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::UNIX_EPOCH);
    (Digest::of(&bytes).to_string(), modified)
}

/// Writes a sidecar beside a recording and returns its path.
fn sidecar_for(path: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let facts = facts_of(path)?;
    let credential = Credential::ephemeral("adiungere.local")?;
    let signed = sign(
        &Request {
            asset: path,
            sources: Vec::new(),
            facts: &facts,
            placement: Placement::Sidecar,
        },
        &credential,
        None,
    )?;
    Ok(signed.written)
}

fn verdict(validator: &Path, file: &Path) -> Result<(String, Vec<String>), String> {
    let verdict = validator_verdict(validator, file)?;
    Ok((verdict.state, verdict.failures))
}

#[test]
fn every_corpus_recording_keeps_its_bytes_and_its_time_and_the_validator_accepts_the_pair() {
    // Arrange
    let validator = pinned_validator().unwrap();
    let directory = Temporary::new("corpus");
    let mut checked = 0usize;

    for spec in corpus().into_iter().filter(|spec| !spec.layout.sparse) {
        let path = directory.0.join(format!("{}.mp4", spec.name));
        build(&spec).unwrap().write_to(&path).unwrap();
        let before = state_of(&path);

        // Act
        let sidecar = sidecar_for(&path).unwrap();
        let after = state_of(&path);
        let (state, failures) = verdict(&validator, &path).unwrap();

        // Assert
        assert_eq!(sidecar, path.with_extension("c2pa"), "{}", spec.name);
        assert_eq!(after.0, before.0, "{}: the recording's bytes changed", spec.name);
        assert_eq!(after.1, before.1, "{}: the recording's time changed", spec.name);
        assert_eq!(state, "Valid", "{}: {failures:?}", spec.name);
        assert_eq!(failures, vec!["signingCredential.untrusted"], "{}", spec.name);
        checked += 1;
    }

    assert!(
        checked >= 10,
        "only {checked} recordings were checked; this check is inert"
    );
}

#[test]
fn the_validator_would_refuse_the_pair_once_one_byte_of_the_recording_changed() {
    // Arrange
    let validator = pinned_validator().unwrap();
    let directory = Temporary::new("changed");
    let path = directory.0.join("reference-like.mp4");
    build(&corpus().into_iter().next().unwrap())
        .unwrap()
        .write_to(&path)
        .unwrap();
    sidecar_for(&path).unwrap();
    let mut bytes = std::fs::read(&path).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    std::fs::write(&path, bytes).unwrap();

    // Act
    let (state, failures) = verdict(&validator, &path).unwrap();

    // Assert
    assert_eq!(state, "Invalid");
    assert!(
        failures.iter().any(|code| code == "assertion.bmffHash.mismatch"),
        "{failures:?}"
    );
}
