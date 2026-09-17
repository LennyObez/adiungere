//! The live time-stamping authority: asked over the network, which no pull-request check does.
//!
//! These tests are ignored by default and run by the nightly pipeline, where a missing answer from the
//! authority is a red job rather than a skipped one. What they hold: a token obtained now names the bytes
//! it was asked about and no others, and a signing with an authority configured carries its time.

use std::path::{Path, PathBuf};

use adiungere_fixtures::{Spec, build};
use adiungere_manifest::{Producer, Scope};
use adiungere_provenance::{Credential, Placement, Request, State, check, read, sign, stamp};

/// The development authority, free and public; the production authority is a configuration value.
const AUTHORITY: &str = "https://freetsa.org/tsr";

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-authority-{name}-{}-{:?}",
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

fn original_in(directory: &Path) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = directory.join("20260604_122323E.MP4");
    build(&Spec::reference_like())?.write_to(&path)?;
    Ok(path)
}

#[test]
#[ignore = "asks a public authority over the network; the nightly pipeline runs it"]
fn the_authority_answers_with_a_token_that_names_the_bytes_and_no_others() {
    // Arrange
    let bytes = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut other = bytes.clone();
    if let Some(byte) = other.get_mut(100) {
        *byte ^= 1;
    }

    // Act
    let token = stamp(&bytes, AUTHORITY).unwrap();
    let attested = check(&token.bytes, &bytes).unwrap();
    let refused = check(&token.bytes, &other);

    // Assert
    assert_eq!(token.authority, AUTHORITY);
    assert_eq!(token.time, attested.time);
    assert!(token.time.ends_with('Z'), "{}", token.time);
    assert!(token.bytes.len() > 1000);
    assert!(refused.is_err(), "{refused:?}");
}

#[test]
#[ignore = "asks a public authority over the network; the nightly pipeline runs it"]
fn a_signing_with_an_authority_carries_its_time_and_still_holds() {
    // Arrange
    let directory = Temporary::new("stamped");
    let original = original_in(&directory.0).unwrap();
    let mut source = adiungere_isobmff::FileSource::open(&original).unwrap();
    let facts = adiungere_manifest::build(
        &mut source,
        "20260604_122323E.MP4",
        Scope::Full,
        &Producer::current("adiungere", "0.0.0"),
        None,
    )
    .unwrap();
    let credential = Credential::ephemeral("adiungere.local")
        .unwrap()
        .stamped_by(AUTHORITY);

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
    assert!(signed.time_stamped);
    assert_eq!(credentials.state, State::ValidNotOnTrustList);
    assert!(
        credentials.time_stamped_at.is_some(),
        "{:?}",
        credentials.statuses
    );
}
