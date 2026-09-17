//! The manifest against the synthetic corpus: built, written, read back, and compared with files that
//! were and were not altered.

use adiungere_fixtures::{Spec, build as build_fixture, corpus};
use adiungere_isobmff::SliceSource;
use adiungere_manifest::{
    Manifest, Outcome, Producer, Scope, Subject, VendorLocation, build, schema, verify,
};

fn producer() -> Producer {
    Producer::current("adiungere", "0.0.0")
}

fn full_manifest(bytes: &[u8]) -> Result<Manifest, adiungere_manifest::Error> {
    let mut source = SliceSource::new(bytes);
    build(&mut source, "clip.mp4", Scope::Full, &producer(), None)
}

fn outcome_for<'a>(
    verification: &'a adiungere_manifest::Verification,
    wanted: &Subject,
) -> Option<&'a Outcome> {
    verification
        .findings
        .iter()
        .find(|finding| finding.subject == *wanted)
        .map(|finding| &finding.outcome)
}

#[test]
fn a_full_manifest_records_every_track_every_vendor_box_and_the_file_digest() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();

    // Act
    let manifest = full_manifest(&bytes).unwrap();

    // Assert
    assert_eq!(manifest.format, "adiungere-manifest/1");
    assert_eq!(manifest.tracks.len(), 3);
    assert!(manifest.tracks.iter().all(|track| track.fingerprint.is_some()));
    assert_eq!(
        manifest
            .tracks
            .iter()
            .filter(|track| track.elementary_stream.is_some())
            .count(),
        2
    );
    assert!(manifest.file.sha256.is_some());
    assert_eq!(manifest.file.size, bytes.len() as u64);
    assert_eq!(manifest.vendor.boxes.len(), 2);
    assert!(!manifest.vendor.interpreted);
    assert_eq!(manifest.file.structure.moov_before_mdat, Some(true));
    assert_eq!(
        manifest.tracks.first().unwrap().codec_string.as_deref(),
        Some("avc1.64000A")
    );
    assert_eq!(manifest.times.len(), 1);
    assert_eq!(manifest.times.first().unwrap().value, "2026-01-01T00:00:00");
}

#[test]
fn a_structure_manifest_reads_no_sample_and_records_no_digest() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let mdat_start = fixture.expected.mdat_offset;
    let mut counting = adiungere_isobmff::CountingSource::new(fixture);

    // Act
    let manifest = build(&mut counting, "clip.mp4", Scope::Structure, &producer(), None).unwrap();

    // Assert
    assert!(manifest.file.sha256.is_none());
    assert!(manifest.tracks.iter().all(|track| track.fingerprint.is_none()));
    assert!(!counting.touched(mdat_start + 16, u64::MAX));
    assert!(counting.bytes_read() < 256 * 1024);
}

#[test]
fn the_canonical_text_reads_back_into_the_same_manifest() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let manifest = full_manifest(&fixture.bytes().unwrap()).unwrap();

    // Act
    let text = manifest.to_canonical_json().unwrap();
    let read = Manifest::from_json(&text).unwrap();

    // Assert
    assert_eq!(read, manifest);
    assert!(text.ends_with('\n'));
}

#[test]
fn an_unknown_field_and_a_foreign_format_are_refused() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let manifest = full_manifest(&fixture.bytes().unwrap()).unwrap();
    let text = manifest.to_canonical_json().unwrap();
    let with_extra = text.replacen("\"format\":", "\"verdict\": \"fine\",\n  \"format\":", 1);
    let foreign = text.replacen("adiungere-manifest/1", "adiungere-manifest/2", 1);

    // Act
    let extra = Manifest::from_json(&with_extra);
    let later = Manifest::from_json(&foreign);

    // Assert
    assert!(extra.is_err());
    assert!(later.is_err());
}

#[test]
fn an_unaltered_file_compares_identical_on_every_subject() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let manifest = full_manifest(&bytes).unwrap();
    let mut source = SliceSource::new(&bytes);

    // Act
    let verification = verify(&manifest, &mut source).unwrap();

    // Assert
    assert!(verification.every_compared_subject_is_identical());
    assert_eq!(verification.findings.len(), 1 + 3 + 2 + 1);
    assert!(
        verification
            .findings
            .iter()
            .all(|finding| finding.outcome == Outcome::Identical)
    );
}

#[test]
fn one_flipped_byte_in_a_sample_changes_that_track_and_the_whole_file_and_nothing_else() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let manifest = full_manifest(&bytes).unwrap();
    // The first sample of the rear track: chunks are interleaved front, rear, audio.
    let front_first_chunk: usize = fixture
        .expected
        .tracks
        .first()
        .unwrap()
        .samples
        .iter()
        .take(15)
        .map(Vec::len)
        .sum();
    let position = usize::try_from(fixture.expected.mdat_offset).unwrap() + 8 + front_first_chunk + 3;
    if let Some(byte) = bytes.get_mut(position) {
        *byte ^= 0x01;
    }
    let mut source = SliceSource::new(&bytes);

    // Act
    let verification = verify(&manifest, &mut source).unwrap();

    // Assert
    assert!(matches!(
        outcome_for(&verification, &Subject::WholeFile),
        Some(Outcome::Differs { .. })
    ));
    assert_eq!(
        outcome_for(&verification, &Subject::Track { index: 0 }),
        Some(&Outcome::Identical)
    );
    assert!(matches!(
        outcome_for(&verification, &Subject::Track { index: 1 }),
        Some(Outcome::Differs { .. })
    ));
    assert_eq!(
        outcome_for(&verification, &Subject::Track { index: 2 }),
        Some(&Outcome::Identical)
    );
    assert_eq!(
        outcome_for(&verification, &Subject::Structure),
        Some(&Outcome::Identical)
    );
    assert!(!verification.every_compared_subject_is_identical());
}

#[test]
fn one_flipped_byte_in_the_vendor_box_changes_that_box_and_the_whole_file_and_nothing_else() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let manifest = full_manifest(&bytes).unwrap();
    let vendor = manifest
        .vendor
        .boxes
        .iter()
        .find(|vendor| vendor.location == VendorLocation::UserData)
        .unwrap();
    let position = usize::try_from(vendor.offset).unwrap() + 100;
    if let Some(byte) = bytes.get_mut(position) {
        *byte ^= 0x80;
    }
    let mut source = SliceSource::new(&bytes);

    // Act
    let verification = verify(&manifest, &mut source).unwrap();

    // Assert
    assert!(matches!(
        outcome_for(&verification, &Subject::WholeFile),
        Some(Outcome::Differs { .. })
    ));
    assert!(matches!(
        outcome_for(
            &verification,
            &Subject::VendorBox {
                location: VendorLocation::UserData,
                kind: "zvnd".to_owned()
            }
        ),
        Some(Outcome::Differs { .. })
    ));
    assert!(
        (0..3)
            .all(|index| outcome_for(&verification, &Subject::Track { index }) == Some(&Outcome::Identical))
    );
    assert_eq!(
        outcome_for(&verification, &Subject::Structure),
        Some(&Outcome::Identical)
    );
}

#[test]
fn a_rewrite_without_the_vendor_boxes_reports_them_missing_and_the_structure_different() {
    // Arrange
    let reference = build_fixture(&Spec::reference_like()).unwrap();
    let manifest = full_manifest(&reference.bytes().unwrap()).unwrap();
    let rewritten = build_fixture(
        &corpus()
            .into_iter()
            .find(|spec| spec.name == "rewritten")
            .unwrap(),
    )
    .unwrap();
    let bytes = rewritten.bytes().unwrap();
    let mut source = SliceSource::new(&bytes);

    // Act
    let verification = verify(&manifest, &mut source).unwrap();

    // Assert
    assert_eq!(
        outcome_for(
            &verification,
            &Subject::VendorBox {
                location: VendorLocation::UserData,
                kind: "zvnd".to_owned()
            }
        ),
        Some(&Outcome::Missing)
    );
    assert!(
        matches!(
            outcome_for(&verification, &Subject::Track { index: 1 }),
            Some(Outcome::NotCheckable { reason }) if reason.contains("mp4a") && reason.contains("avc1")
        ),
        "the rear track is gone and the audio track moved into its place: {:?}",
        outcome_for(&verification, &Subject::Track { index: 1 })
    );
    assert_eq!(
        outcome_for(&verification, &Subject::Track { index: 2 }),
        Some(&Outcome::Missing)
    );
    assert!(matches!(
        outcome_for(&verification, &Subject::Structure),
        Some(Outcome::Differs { .. })
    ));
}

#[test]
fn a_vendor_box_is_matched_by_its_type_and_its_location_together() {
    // Arrange: the same recording with the user-data child renamed. The recorded box is then missing and
    // the renamed one is unexpected; neither is compared with the other.
    let reference = build_fixture(&Spec::reference_like()).unwrap();
    let manifest = full_manifest(&reference.bytes().unwrap()).unwrap();
    let mut bytes = reference.bytes().unwrap();
    let header = bytes
        .windows(4)
        .position(|window| window == adiungere_fixtures::VENDOR_BOX)
        .unwrap();
    bytes
        .get_mut(header..header + 4)
        .unwrap()
        .copy_from_slice(b"zvnx");
    let mut source = SliceSource::new(&bytes);

    // Act
    let verification = verify(&manifest, &mut source).unwrap();

    // Assert
    assert_eq!(
        outcome_for(
            &verification,
            &Subject::VendorBox {
                location: VendorLocation::UserData,
                kind: "zvnd".to_owned()
            }
        ),
        Some(&Outcome::Missing)
    );
    assert_eq!(
        outcome_for(
            &verification,
            &Subject::VendorBox {
                location: VendorLocation::UserData,
                kind: "zvnx".to_owned()
            }
        ),
        Some(&Outcome::Unexpected)
    );
    assert_eq!(
        outcome_for(
            &verification,
            &Subject::VendorBox {
                location: VendorLocation::TopLevel,
                kind: "zzzz".to_owned()
            }
        ),
        Some(&Outcome::Identical)
    );
    assert!(!verification.every_compared_subject_is_identical());
}

#[test]
fn the_structure_lists_each_encoder_tag_once_and_marks_a_standard_user_data_box() {
    // Arrange: two video tracks share a compressor name and a handler name.
    let reference = build_fixture(&Spec::reference_like()).unwrap();
    let mut bytes = reference.bytes().unwrap();
    let plain = full_manifest(&bytes).unwrap();
    let header = bytes
        .windows(4)
        .position(|window| window == adiungere_fixtures::VENDOR_BOX)
        .unwrap();
    bytes
        .get_mut(header..header + 4)
        .unwrap()
        .copy_from_slice(b"meta");

    // Act
    let renamed = full_manifest(&bytes).unwrap();

    // Assert
    assert_eq!(
        plain.file.structure.encoder_tags,
        vec![
            "synthetic pattern".to_owned(),
            "VideoHandler".to_owned(),
            "SoundHandler".to_owned()
        ]
    );
    assert!(plain.vendor.boxes.iter().all(|vendor| !vendor.standard));
    let standard: Vec<&str> = renamed
        .vendor
        .boxes
        .iter()
        .filter(|vendor| vendor.standard)
        .map(|vendor| vendor.kind.as_str())
        .collect();
    assert_eq!(standard, vec!["meta"]);
    assert_eq!(
        renamed.vendor.boxes.len(),
        2,
        "a standard box is still preserved and digested"
    );
}

#[test]
fn an_error_says_what_went_wrong_and_keeps_its_cause() {
    // Arrange
    let mut source = SliceSource::new(b"not a container");

    // Act
    let error = build(&mut source, "x.mp4", Scope::Full, &producer(), None).unwrap_err();

    // Assert
    assert!(error.to_string().contains("box header"), "{error}");
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn a_manifest_without_fingerprints_compares_nothing_it_did_not_record() {
    // Arrange
    let fixture = build_fixture(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let mut source = SliceSource::new(&bytes);
    let manifest = build(&mut source, "clip.mp4", Scope::Structure, &producer(), None).unwrap();

    // Act
    let verification = verify(&manifest, &mut source).unwrap();

    // Assert
    assert_eq!(
        outcome_for(&verification, &Subject::WholeFile),
        Some(&Outcome::NotRecorded)
    );
    assert!(
        (0..3).all(|index| outcome_for(&verification, &Subject::Track { index })
            == Some(&Outcome::NotRecorded))
    );
    assert!(verification.every_compared_subject_is_identical());
}

#[test]
fn the_published_schema_is_the_one_the_types_derive() {
    // The schema is committed so a reader can validate a manifest without this code. Committing it means
    // it can drift; this keeps it honest, and writes it when asked to.

    // Arrange
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("schema/adiungere-manifest-1.json");
    let derived = schema().unwrap();

    if std::env::var_os("ADIUNGERE_WRITE_SCHEMA").is_some() {
        std::fs::write(&path, &derived).unwrap();
    }

    // Act
    let committed = std::fs::read_to_string(&path).unwrap();

    // Assert
    assert_eq!(
        committed, derived,
        "schema/adiungere-manifest-1.json is out of date; run the test with ADIUNGERE_WRITE_SCHEMA=1"
    );
    assert!(derived.contains("\"$schema\""));
    assert!(derived.contains("additionalProperties"));
}
