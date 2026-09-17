//! **G30.** The signer identity a surface shows is the certificate's subject, and the trust state it
//! shows is the validator's result, in every state the validator can return.
//!
//! The one thing worse than no credentials is credentials described as more than they are. So the
//! product's own reading of a signed file is held against the pinned validator's: the common name it
//! prints is the one in the certificate the validator read; a chain on no trust list is shown as exactly
//! that, never as trusted; a broken binding is shown as invalid; and the words the command line prints
//! come from the catalogue, so the wording guarantee covers them.

use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use adiungere_cli::media::{self, Output, Selection};
use adiungere_cli::provenance::{Credentialling, Signing};
use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::{pinned_validator, validator_verdict};
use adiungere_isobmff::{Input, RemuxPlan, SliceSource, parse, remux};
use adiungere_provenance::State;

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g30-{name}-{}-{:?}",
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

/// A signed rear-only export of the reference-like recording, with its manifest beside it.
fn signed_export(directory: &Path) -> Result<(PathBuf, PathBuf), Box<dyn std::error::Error>> {
    let recording = directory.join("20260604_122323E.MP4");
    build(&Spec::reference_like())?.write_to(&recording)?;
    let signed = directory.join("rear.mp4");
    let signing = Signing {
        credentialling: Credentialling::Ephemeral,
        time_authority: None,
    };
    media::export(
        &[recording],
        &Selection::Rear,
        &signed,
        None,
        Some(&signing),
        Output::Json,
        &AtomicBool::new(false),
    )?;
    Ok((signed, directory.join("rear.mp4.manifest.json")))
}

/// A remux of every track of a file, written under another name: the samples kept, the binding broken.
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
    let plan = RemuxPlan {
        keep_manifest_store: true,
        ..RemuxPlan::default()
    };
    let mut out = Vec::new();
    remux(&mut inputs, &plan, &mut out, &mut |_| true)?;
    std::fs::write(to, out)?;
    Ok(())
}

/// What the validator says: its state, the signer's common name, and its failure codes.
fn validator_says(validator: &Path, file: &Path) -> Result<(String, String, Vec<String>), String> {
    let verdict = validator_verdict(validator, file)?;
    Ok((verdict.state, verdict.common_name, verdict.failures))
}

#[test]
fn the_identity_and_the_state_the_product_shows_are_the_validators() {
    // Arrange
    let validator = pinned_validator().unwrap();
    let directory = Temporary::new("states");
    let (signed, manifest) = signed_export(&directory.0).unwrap();
    let broken = directory.0.join("broken.mp4");
    remuxed(&signed, &broken).unwrap();

    // Act
    let (validator_state, validator_name, validator_failures) = validator_says(&validator, &signed).unwrap();
    let (broken_state, _, broken_failures) = validator_says(&validator, &broken).unwrap();
    let ours = adiungere_provenance::read(&signed, None).unwrap();
    let ours_broken = adiungere_provenance::read(&broken, None).unwrap();
    let printed = media::verify(&manifest, &signed, Output::Text).unwrap();
    let printed_broken = media::verify(&manifest, &broken, Output::Text).unwrap();

    // Assert: the name.
    assert_eq!(validator_name, "adiungere.local");
    assert_eq!(
        ours.signer.as_ref().and_then(|s| s.common_name.as_deref()),
        Some(validator_name.as_str())
    );
    assert!(
        printed.text.contains("signed as adiungere.local"),
        "{}",
        printed.text
    );

    // Assert: the state, with a chain on no trust list shown as that and never as more.
    assert_eq!(validator_state, "Valid");
    assert_eq!(validator_failures, vec!["signingCredential.untrusted"]);
    assert_eq!(ours.state, State::ValidNotOnTrustList);
    assert!(
        printed.text.contains("chains to no trust list"),
        "{}",
        printed.text
    );
    assert!(!printed.negative);
    assert_eq!(broken_state, "Invalid");
    assert!(
        broken_failures
            .iter()
            .any(|code| code == "assertion.bmffHash.mismatch")
    );
    assert_eq!(ours_broken.state, State::Invalid);
    assert!(
        printed_broken.text.contains("does not hold: ")
            && printed_broken.text.contains("assertion.bmffHash.mismatch"),
        "{}",
        printed_broken.text
    );
    assert!(printed_broken.negative);
    let states_shown: Vec<&str> = [&printed.text, &printed_broken.text]
        .iter()
        .flat_map(|text| text.lines())
        .filter(|line| line.contains("The signature"))
        .collect();
    assert_eq!(states_shown.len(), 4, "{states_shown:?}");
}

#[test]
fn the_states_the_product_can_show_are_exactly_the_validators_three() {
    // The product's state type has one variant per validator state, so no fourth word can appear.

    // Act
    let shown: Vec<String> = [State::Trusted, State::ValidNotOnTrustList, State::Invalid]
        .iter()
        .map(|state| serde_json::to_string(state).unwrap())
        .collect();

    // Assert
    assert_eq!(
        shown,
        vec![
            "\"trusted\"".to_owned(),
            "\"valid_not_on_trust_list\"".to_owned(),
            "\"invalid\"".to_owned()
        ]
    );
}
