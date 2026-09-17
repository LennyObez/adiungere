//! **G29.** The report, the Content Credentials manifest and the adiungere manifest agree on every digest
//! and every action.
//!
//! Three documents describe one signed export: the adiungere manifest written beside it, the facts the
//! signed claim carries as the integrity assertion, and the report rendered from the manifest for a
//! person. A stranger reads the second with the pinned validator and the other two with this product,
//! and the numbers have to be the same numbers. This guarantee signs an export through the command line's
//! own functions, reads the assertion back through the validator, and holds that every track digest,
//! every source digest and every action agree across the three, and that the action follows from the
//! export class and from nothing else.

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

use adiungere_cli::media::{self, Output, Selection};
use adiungere_cli::provenance::{Credentialling, Signing};
use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::{at, pinned_validator, text_at, validator_verdict};
use adiungere_manifest::{ExportClass, Masking, Operation, Producer, Scope};
use adiungere_provenance::{Credential, Placement, Request};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g29-{name}-{}-{:?}",
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

/// The digests of every track of a manifest as JSON, payload and configuration, in track order.
fn track_digests(manifest: &serde_json::Value) -> Vec<(String, String)> {
    at(manifest, &["tracks"])
        .and_then(serde_json::Value::as_array)
        .map(|tracks| {
            tracks
                .iter()
                .map(|track| {
                    (
                        text_at(track, &["fingerprint", "payload_sha256"]),
                        text_at(track, &["fingerprint", "configuration_sha256"]),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// The sources of a manifest as JSON: name and digest, in order.
fn sources_of(manifest: &serde_json::Value) -> Vec<(String, String)> {
    at(manifest, &["produced", "operation", "sources"])
        .and_then(serde_json::Value::as_array)
        .map(|sources| {
            sources
                .iter()
                .map(|source| (text_at(source, &["name"]), text_at(source, &["sha256"])))
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn every_digest_and_every_action_is_the_same_in_the_three_documents() {
    // Arrange
    let validator = pinned_validator().unwrap();
    let directory = Temporary::new("agreement");
    let recording = directory.0.join("20260604_122323E.MP4");
    build(&Spec::reference_like())
        .unwrap()
        .write_to(&recording)
        .unwrap();
    let signed = directory.0.join("rear.mp4");
    let signing = Signing {
        credentialling: Credentialling::Ephemeral,
        time_authority: None,
    };
    let exported = media::export(
        std::slice::from_ref(&recording),
        &Selection::Rear,
        &signed,
        None,
        Some(&signing),
        Output::Json,
        &AtomicBool::new(false),
    )
    .unwrap();
    let export_answer: serde_json::Value = serde_json::from_str(&exported.text).unwrap();
    let beside_path = directory.0.join("rear.mp4.manifest.json");

    // Act
    let beside: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&beside_path).unwrap()).unwrap();
    let verdict = validator_verdict(&validator, &signed).unwrap();
    let assertion = verdict.integrity.clone().unwrap();
    let report_text = media::report(&beside_path, Output::Text).unwrap().text;
    let recording_manifest: serde_json::Value =
        serde_json::from_str(&media::fingerprint(&recording, Output::Json, None).unwrap().text).unwrap();
    let recording_digest = text_at(&recording_manifest, &["file", "sha256"]);
    let read_back = at(&export_answer, &["credentials", "credentials", "integrity"])
        .cloned()
        .unwrap();

    // Assert: the track digests are one set of numbers.
    let signed_digests = track_digests(&assertion);
    assert_eq!(signed_digests.len(), 2);
    assert!(signed_digests.iter().all(|(p, c)| p.len() == 64 && c.len() == 64));
    assert_eq!(
        track_digests(&beside),
        signed_digests,
        "the manifest beside the file"
    );
    assert_eq!(
        track_digests(&read_back),
        signed_digests,
        "the facts the product read back"
    );
    for (payload, configuration) in &signed_digests {
        assert!(report_text.contains(payload), "the report lacks {payload}");
        assert!(
            report_text.contains(configuration),
            "the report lacks {configuration}"
        );
    }

    // Assert: the sources are the recording, by its digest, in both manifests.
    assert_eq!(recording_digest.len(), 64);
    let expected_sources = vec![("20260604_122323E.MP4".to_owned(), recording_digest)];
    assert_eq!(sources_of(&assertion), expected_sources, "the signed facts");
    assert_eq!(
        sources_of(&beside),
        expected_sources,
        "the manifest beside the file"
    );

    // Assert: the actions follow from the class, and the product reads the same ones.
    assert_eq!(
        text_at(&assertion, &["produced", "operation", "class"]),
        "extraction"
    );
    assert_eq!(
        text_at(&beside, &["produced", "operation", "class"]),
        "extraction"
    );
    assert_eq!(verdict.actions, vec!["c2pa.opened", "c2pa.repackaged"]);
    assert_eq!(
        at(&export_answer, &["credentials", "credentials", "actions"]),
        Some(&serde_json::json!(["c2pa.opened", "c2pa.repackaged"]))
    );
}

#[test]
fn the_action_follows_from_the_export_class_and_from_nothing_else() {
    // A stream copy is a repackaging, a re-encoding is a transcoding, and a recording signed as it is was
    // only opened. Each class is signed as a sidecar over the same bytes, and the validator reads the
    // action back.

    // Arrange
    let validator = pinned_validator().unwrap();
    let directory = Temporary::new("classes");
    let producer = Producer::current("adiungere", "0.0.0");
    let cases: Vec<(&str, Option<ExportClass>, Vec<&str>)> = vec![
        ("recording", None, vec!["c2pa.opened"]),
        (
            "extraction",
            Some(ExportClass::Extraction),
            vec!["c2pa.opened", "c2pa.repackaged"],
        ),
        (
            "archive",
            Some(ExportClass::TwoTrackArchive),
            vec!["c2pa.opened", "c2pa.repackaged"],
        ),
        (
            "lossy",
            Some(ExportClass::RenditionLossy),
            vec!["c2pa.opened", "c2pa.transcoded"],
        ),
        (
            "lossless",
            Some(ExportClass::RenditionLosslessVerified),
            vec!["c2pa.opened", "c2pa.transcoded"],
        ),
    ];

    for (name, class, expected) in cases {
        let path = directory.0.join(format!("{name}.mp4"));
        build(&Spec::reference_like()).unwrap().write_to(&path).unwrap();
        let mut source = adiungere_isobmff::FileSource::open(&path).unwrap();
        let facts = match class {
            None => {
                adiungere_manifest::build(&mut source, &format!("{name}.mp4"), Scope::Full, &producer, None)
            },
            Some(class) => adiungere_manifest::build_for_export(
                &mut source,
                &format!("{name}.mp4"),
                &producer,
                None,
                Operation::Export {
                    class,
                    masking: Masking::None,
                    sources: Vec::new(),
                },
            ),
        }
        .unwrap();
        let credential = Credential::ephemeral("adiungere.local").unwrap();

        // Act
        adiungere_provenance::sign(
            &Request {
                asset: &path,
                sources: Vec::new(),
                facts: &facts,
                placement: Placement::Sidecar,
            },
            &credential,
            None,
        )
        .unwrap();
        let verdict = validator_verdict(&validator, &path).unwrap();

        // Assert
        assert_eq!(verdict.actions, expected, "{name}");
    }
}
