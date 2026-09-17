//! **G01.** No tracked file carries an em dash or an en dash, in any form a renderer would show as one.
//!
//! Long dashes read as machine-written prose. That perception is not universal and it may not last, but this
//! repository is read by people deciding whether its author writes carefully, and the cost of avoiding them
//! is a comma, a colon or a second sentence. Ordinary punctuation is never the weaker choice.
//!
//! The forbidden characters are written as escapes rather than as themselves, so this file holds the rule
//! without holding what the rule forbids. The rule covers the characters, their look-alikes, and the HTML
//! entities a Markdown renderer turns into them, because a dash that reaches the reader through an entity
//! is still a dash.

use adiungere_guarantees::{TextFile, tracked_text_files};

/// The characters this guarantee refuses, by name.
const CHARACTERS: [(&str, char); 5] = [
    ("em dash", '\u{2014}'),
    ("en dash", '\u{2013}'),
    ("horizontal bar", '\u{2015}'),
    ("two-em dash", '\u{2E3A}'),
    ("small em dash", '\u{FE58}'),
];

/// The HTML entities a renderer shows as one of the characters above, assembled so that this file does not
/// contain them.
fn entities() -> [String; 6] {
    ["mdash;", "ndash;", "#8212;", "#8211;", "#x2014;", "#x2013;"].map(|name| format!("&{name}"))
}

fn carries_a_forbidden_dash(text: &str) -> bool {
    CHARACTERS.iter().any(|(_, character)| text.contains(*character))
        || entities().iter().any(|entity| text.contains(entity.as_str()))
}

fn offenders(files: &[TextFile]) -> Vec<String> {
    let mut found: Vec<String> = files
        .iter()
        .filter(|file| carries_a_forbidden_dash(&file.contents))
        .map(|file| file.path.clone())
        .collect();

    found.sort();
    found
}

#[test]
fn no_tracked_file_carries_a_long_dash() {
    // Arrange
    let files = tracked_text_files().unwrap();

    // Act
    let found = offenders(&files);

    // Assert
    assert!(
        found.is_empty(),
        "These tracked files carry a long dash, as a character or as an entity. Replace it with a comma, a \
         colon, or a second sentence:\n  {}",
        found.join("\n  ")
    );
}

#[test]
fn the_check_detects_every_character_it_forbids() {
    // A check built from escapes passes silently if an escape is wrong, so each is exercised against a
    // string it must catch.

    // Act
    let undetected: Vec<&str> = CHARACTERS
        .iter()
        .filter(|(_, character)| !carries_a_forbidden_dash(&format!("before {character} after")))
        .map(|(name, _)| *name)
        .collect();

    // Assert
    assert!(
        undetected.is_empty(),
        "These characters were not detectable: {undetected:?}"
    );
}

#[test]
fn the_check_detects_every_entity_it_forbids() {
    // Act
    let all = entities();
    let undetected: Vec<&String> = all
        .iter()
        .filter(|entity| !carries_a_forbidden_dash(&format!("a {entity} b")))
        .collect();

    // Assert
    assert!(
        undetected.is_empty(),
        "These entities were not detectable: {undetected:?}"
    );
}

#[test]
fn an_ordinary_hyphen_is_not_flagged() {
    // The complement: a check that also caught the hyphen would be unusable, since compound words need it.

    // Arrange
    let innocent = "a well-known state-of-the-art two-track file, with an &amp; entity";

    // Act
    let flagged = carries_a_forbidden_dash(innocent);

    // Assert
    assert!(!flagged, "An ordinary hyphen must not be flagged.");
}

#[test]
fn the_scan_reads_real_tracked_files() {
    // Without this, a change to how tracked files are listed would let the check above pass by reading
    // nothing at all.

    // Act
    let files = tracked_text_files().unwrap();

    // Assert
    assert!(
        files.len() > 50,
        "Only {} tracked files were read; the scan is not reading the repository.",
        files.len()
    );
    assert!(
        files.iter().any(|file| file.path == "README.md"),
        "The scan did not reach the README, so it is not reading the repository."
    );
}
