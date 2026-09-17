//! The fingerprints against the synthetic corpus, and against what the external tool produced.
//!
//! The elementary stream digests below were measured with the maintainer's copy of the common
//! command-line tool on the corpus written by `adiungere-fixtures`, with the command the integrity
//! specification documents. They are the oracle: if the rule in this crate drifts from the tool, these
//! tests go red before anyone reads a digest that nobody else can reproduce.

use adiungere_fingerprint::{
    ANNEX_B_RULE, Digest, TRACK_FINGERPRINT_VERSION, annex_b_digest, file_digest, structural_fingerprint,
    track_fingerprint,
};
use adiungere_fixtures::{Role, Spec, build, corpus};
use adiungere_isobmff::{SliceSource, parse};
use sha2::{Digest as _, Sha256};

/// The digests the tool produced for the front and rear tracks, identical across every prefix width.
const FRONT_STREAM: &str = "18c7cf5b056783346aac795d2f3ad9c7b138dcbe1df0dc74e77c9cfe55fc8505";
const REAR_STREAM: &str = "6b4bc22f3aeae97baab7a7f89db69d31c5fb003aa2317be26638facaf7fa51a2";

#[test]
fn the_track_fingerprint_is_the_digest_of_the_stored_samples_and_the_configuration() {
    for spec in corpus() {
        // Arrange
        let mut fixture = build(&spec).unwrap();
        let container = parse(&mut fixture).unwrap();
        let tracks = container.tracks().unwrap();

        for (index, expected) in fixture.expected.tracks.clone().iter().enumerate() {
            // The expectation is assembled from the frames the builder wrote, not from the reader.
            let mut hasher = Sha256::new();
            for sample in &expected.samples {
                hasher.update(sample);
            }
            let expected_payload = Digest(hasher.finalize().into());
            let expected_configuration = Digest::of(&expected.configuration_payload);

            // Act
            let fingerprint =
                track_fingerprint(&mut fixture, &container, tracks.get(index).unwrap()).unwrap();

            // Assert
            assert_eq!(fingerprint.version, TRACK_FINGERPRINT_VERSION);
            assert_eq!(
                fingerprint.payload_sha256, expected_payload,
                "{} track {index}",
                spec.name
            );
            assert_eq!(fingerprint.configuration_sha256, expected_configuration);
            assert_eq!(fingerprint.sample_count as usize, expected.samples.len());
            let nal = match expected.role {
                Role::Front | Role::Rear => Some(spec.nal_length_size),
                Role::Audio => None,
            };
            assert_eq!(fingerprint.nal_length_size, nal, "{} track {index}", spec.name);
        }
    }
}

#[test]
fn the_elementary_stream_digest_equals_what_the_tool_produces() {
    for spec in corpus() {
        if !spec.tracks.rear {
            continue;
        }
        // Arrange
        let mut fixture = build(&spec).unwrap();
        let container = parse(&mut fixture).unwrap();
        let tracks = container.tracks().unwrap();

        // Act
        let front = annex_b_digest(&mut fixture, &container, tracks.first().unwrap()).unwrap();
        let rear = annex_b_digest(&mut fixture, &container, tracks.get(1).unwrap()).unwrap();

        // Assert
        assert_eq!(front.sha256.to_string(), FRONT_STREAM, "{}", spec.name);
        assert_eq!(rear.sha256.to_string(), REAR_STREAM, "{}", spec.name);
        assert_eq!(front.rule, ANNEX_B_RULE);
        assert_eq!(front.stream_bytes, 3390);
        assert_eq!(rear.stream_bytes, 1092);
        assert_eq!(front.access_units, 30);
        assert_eq!(front.parameter_set_insertions, 2);
    }
}

#[test]
fn the_stream_digest_does_not_depend_on_the_prefix_width_and_the_track_fingerprint_does() {
    // Arrange
    let mut wide = build(&Spec::reference_like()).unwrap();
    let narrow_spec = corpus()
        .into_iter()
        .find(|spec| spec.nal_length_size == 1)
        .unwrap();
    let mut narrow = build(&narrow_spec).unwrap();
    let wide_container = parse(&mut wide).unwrap();
    let narrow_container = parse(&mut narrow).unwrap();
    let wide_track = wide_container.tracks().unwrap().remove(0);
    let narrow_track = narrow_container.tracks().unwrap().remove(0);

    // Act
    let wide_stream = annex_b_digest(&mut wide, &wide_container, &wide_track).unwrap();
    let narrow_stream = annex_b_digest(&mut narrow, &narrow_container, &narrow_track).unwrap();
    let wide_fingerprint = track_fingerprint(&mut wide, &wide_container, &wide_track).unwrap();
    let narrow_fingerprint = track_fingerprint(&mut narrow, &narrow_container, &narrow_track).unwrap();

    // Assert
    assert_eq!(wide_stream.sha256, narrow_stream.sha256);
    assert_ne!(wide_fingerprint.payload_sha256, narrow_fingerprint.payload_sha256);
}

#[test]
fn the_audio_track_has_no_elementary_stream_digest_and_the_refusal_names_the_track() {
    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();
    let container = parse(&mut fixture).unwrap();
    let audio = container.tracks().unwrap().remove(2);

    // Act
    let error = annex_b_digest(&mut fixture, &container, &audio).unwrap_err();

    // Assert
    assert_eq!(
        error.to_string(),
        "track 2 is not advanced video coding, for which the stream digest is defined"
    );
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn a_source_failure_is_carried_as_the_cause_of_the_fingerprint_error() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let cut = usize::try_from(fixture.expected.mdat_offset + 64).unwrap();
    let mut source = SliceSource::new(&bytes[..cut]);
    let container = parse(&mut source).unwrap();

    // Act
    let error = annex_b_digest(
        &mut source,
        &container,
        container.tracks().unwrap().first().unwrap(),
    )
    .unwrap_err();

    // Assert
    assert!(error.to_string().contains("the source holds"), "{error}");
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn the_file_digest_is_the_digest_of_every_byte() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let mut source = SliceSource::new(&bytes);

    // Act
    let (digest, length) = file_digest(&mut source).unwrap();

    // Assert
    assert_eq!(digest, Digest::of(&bytes));
    assert_eq!(length, bytes.len() as u64);
}

#[test]
fn a_truncated_recording_is_inspected_and_its_fingerprint_is_refused_with_the_reason() {
    // A recorder that lost power wrote the movie box and part of the media data it describes.

    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    // The cut falls inside the media data, after its header and before its last byte.
    let cut = usize::try_from(fixture.expected.mdat_offset + 64).unwrap();
    assert!(fixture.expected.mdat_size > 64);
    let mut source = SliceSource::new(&bytes[..cut]);
    let container = parse(&mut source).unwrap();

    // Act
    let outcome = track_fingerprint(
        &mut source,
        &container,
        container.tracks().unwrap().first().unwrap(),
    );

    // Assert
    assert_eq!(
        container.clipped_tail().map(|tail| tail.kind.to_string()),
        Some("mdat".to_owned())
    );
    let message = outcome.unwrap_err().to_string();
    assert!(
        message.contains("were asked for and the source holds"),
        "{message}"
    );
    assert!(
        structural_fingerprint(&container)
            .observations
            .iter()
            .any(|line| line.starts_with("top-level-clipped mdat")),
        "{:?}",
        structural_fingerprint(&container).observations
    );
}

#[test]
fn the_structural_fingerprint_separates_a_rewrite_from_its_source_and_not_a_prefix_width() {
    // Arrange
    let mut reference = build(&Spec::reference_like()).unwrap();
    let mut narrow = build(
        &corpus()
            .into_iter()
            .find(|spec| spec.nal_length_size == 1)
            .unwrap(),
    )
    .unwrap();
    let mut rewritten = build(
        &corpus()
            .into_iter()
            .find(|spec| spec.name == "rewritten")
            .unwrap(),
    )
    .unwrap();

    // Act
    let reference_shape = structural_fingerprint(&parse(&mut reference).unwrap());
    let narrow_shape = structural_fingerprint(&parse(&mut narrow).unwrap());
    let rewritten_shape = structural_fingerprint(&parse(&mut rewritten).unwrap());

    // Assert
    assert_eq!(reference_shape.sha256, narrow_shape.sha256);
    assert_ne!(reference_shape.sha256, rewritten_shape.sha256);
    assert!(
        reference_shape
            .observations
            .iter()
            .any(|line| line.starts_with("udta zvnd 30720"))
    );
    assert!(
        reference_shape
            .observations
            .iter()
            .any(|line| line.starts_with("top-level-unknown zzzz 12"))
    );
}
