//! **G01.** No tracked file carries an em dash or an en dash.
//!
//! Long dashes read as machine-written prose. That perception is not universal and it may not last, but this
//! repository is read by people deciding whether its author writes carefully, and the cost of avoiding them
//! is a comma, a colon or a second sentence. Ordinary punctuation is never the weaker choice.
//!
//! The forbidden characters are built from their code points rather than written out, so this file does not
//! contain what it forbids. Writing them here would force the check either to fail on itself or to exempt
//! itself, and a guarantee with an exemption for its own file is not a guarantee.

use adiungere_guarantees::{TextFile, tracked_text_files};

/// The characters this guarantee refuses, by name.
///
/// Each is written as an escape rather than as itself, so this file holds the rule without holding what the
/// rule forbids.
const FORBIDDEN: [(&str, char); 2] = [("em dash", '\u{2014}'), ("en dash", '\u{2013}')];

fn carries_a_forbidden_character(text: &str) -> bool {
    FORBIDDEN.iter().any(|(_, character)| text.contains(*character))
}

fn offenders(files: &[TextFile]) -> Vec<String> {
    let mut found: Vec<String> = files
        .iter()
        .filter(|file| carries_a_forbidden_character(&file.contents))
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
        "These tracked files carry a long dash. Replace it with a comma, a colon, or a second \
         sentence:\n  {}",
        found.join("\n  ")
    );
}

#[test]
fn the_check_detects_the_characters_it_forbids() {
    // A check built from code points passes silently if a code point is wrong, so each is exercised against
    // a string it must catch.

    // Arrange
    let characters = FORBIDDEN;

    // Act
    let undetected: Vec<&str> = characters
        .iter()
        .filter(|(_, character)| !carries_a_forbidden_character(&format!("before {character} after")))
        .map(|(name, _)| *name)
        .collect();

    // Assert
    assert!(
        undetected.is_empty(),
        "These characters were not detectable: {undetected:?}"
    );
    assert_eq!(
        characters.len(),
        2,
        "Both the em dash and the en dash must be covered."
    );
}

#[test]
fn an_ordinary_hyphen_is_not_flagged() {
    // The complement: a check that also caught the hyphen would be unusable, since compound words need it.

    // Arrange
    let innocent = "a well-known state-of-the-art two-track file";

    // Act
    let flagged = carries_a_forbidden_character(innocent);

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
        !files.is_empty(),
        "No tracked file was read; the check above is inert."
    );
    assert!(
        files.iter().any(|file| file.path == "README.md"),
        "The scan did not reach the README, so it is not reading the repository."
    );
}
