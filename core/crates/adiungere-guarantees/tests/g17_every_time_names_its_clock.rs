//! **G17.** Every time in a manifest names the clock it came from, out of a closed set, and every sentence
//! the product forms about a time says "no later than" and names that clock.
//!
//! A recording carries several clocks and they disagree with each other; the reference recorder writes a
//! local session clock into a field defined as universal time. A bare timestamp in a claim file is a
//! number a person will trust, so no bare timestamp exists here: a time is a value, a clock from a closed
//! set, and a note on what the clock is worth.

use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::read_at;
use adiungere_isobmff::SliceSource;
use adiungere_manifest::time::TimeSource;
use adiungere_manifest::wording::{ENGLISH, parse_catalogue};
use adiungere_manifest::{Producer, Scope, schema};

/// The clocks, as the schema must list them and no others.
const CLOCKS: [&str; 4] = [
    "container_clock",
    "satellite_clock",
    "file_name",
    "system_clock_at_manifest",
];

#[test]
fn the_published_schema_closes_the_set_of_clocks_and_requires_one_on_every_time() {
    // Arrange
    let published: serde_json::Value = serde_json::from_str(
        &read_at("core/crates/adiungere-manifest/schema/adiungere-manifest-1.json").unwrap(),
    )
    .unwrap();
    let derived: serde_json::Value = serde_json::from_str(&schema().unwrap()).unwrap();

    // Act
    let schema = &published;
    let sources = schema["$defs"]["TimeSource"]["oneOf"].as_array().unwrap();
    let listed: Vec<&str> = sources
        .iter()
        .filter_map(|variant| variant["const"].as_str())
        .collect();
    let required = schema["$defs"]["TimeValue"]["required"].as_array().unwrap();
    let required: Vec<&str> = required.iter().filter_map(serde_json::Value::as_str).collect();

    // Assert
    assert_eq!(published, derived, "the published schema is not the derived one");
    assert_eq!(listed, CLOCKS);
    assert!(required.contains(&"source") && required.contains(&"value") && required.contains(&"note"));
    assert_eq!(
        schema["$defs"]["TimeValue"]["additionalProperties"],
        serde_json::Value::Bool(false)
    );
}

#[test]
fn every_time_in_a_built_manifest_carries_a_clock_and_a_note() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let mut source = SliceSource::new(&bytes);
    let file_name_time = adiungere_manifest::time::Civil::from_unix_seconds(1_767_225_600);

    // Act
    let manifest = adiungere_manifest::build(
        &mut source,
        "20260101_000000E.MP4",
        Scope::Structure,
        &Producer::current("test", "0"),
        Some(file_name_time),
    )
    .unwrap();

    // Assert
    assert!(manifest.times.len() >= 2);
    for time in &manifest.times {
        assert!(!time.note.is_empty(), "{time:?} has no note");
        assert!(!time.value.is_empty());
    }
    let sources: Vec<TimeSource> = manifest.times.iter().map(|time| time.source).collect();
    assert_eq!(sources, vec![TimeSource::ContainerClock, TimeSource::FileName]);
    assert_eq!(manifest.produced.at.source, TimeSource::SystemClockAtManifest);
    assert!(
        manifest.produced.at.value.ends_with('Z'),
        "the system clock is read as universal time"
    );
    assert!(
        !manifest.times.first().unwrap().value.ends_with('Z'),
        "the recorder's clock is not claimed as universal time"
    );
}

#[test]
fn every_time_sentence_says_no_later_than_and_names_the_clock() {
    // Arrange
    let catalogue = parse_catalogue(ENGLISH).unwrap();

    // Act
    let sentences: Vec<(&String, &String)> = catalogue
        .strings
        .iter()
        .filter(|(key, _)| key.starts_with("time.") && !key.contains(".note") && *key != "time.disagreement")
        .collect();
    let notes: Vec<(&String, &String)> = catalogue
        .strings
        .iter()
        .filter(|(key, _)| key.starts_with("time.") && key.contains(".note"))
        .collect();

    // Assert
    assert!(
        !sentences.is_empty(),
        "no time sentence was found; this check is inert"
    );
    for (key, text) in sentences {
        assert!(text.contains("no later than"), "{key}: {text}");
        assert!(text.contains("{clock}"), "{key}: {text}");
    }
    assert_eq!(
        notes.len(),
        CLOCKS.len(),
        "every clock has a note explaining what it is worth"
    );
    assert!(
        TimeSource::ContainerClock.describe().contains("clock")
            && TimeSource::FileName.describe().contains("file name")
    );
}
