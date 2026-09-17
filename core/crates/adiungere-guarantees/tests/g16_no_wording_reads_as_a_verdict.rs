//! **G16.** No string in any wording catalogue declares a verdict or uses the jargon of the format.
//!
//! The product reports facts and never decides whether a recording is what it claims to be. The sentences
//! it forms live in catalogues, one per language, and every catalogue has a list of the words that would
//! turn a fact into a verdict. Each word is refused as a whole word, without regard to case, so that
//! "verified" is caught and "the verifier" is not. The languages are discovered from the directory, so a
//! catalogue added for a new language is checked from the day it exists.

use adiungere_guarantees::{TextFile, tracked_text_files_under};
use adiungere_manifest::wording::{contains_forbidden_word, parse_catalogue, parse_forbidden};

/// Every catalogue with its forbidden-words list, or the first catalogue that lacks one.
fn catalogues() -> Result<Vec<(String, TextFile, TextFile)>, String> {
    let files = tracked_text_files_under("core/crates/").map_err(|error| error.to_string())?;
    let mut found = Vec::new();
    for file in &files {
        let Some((directory, name)) = file.path.rsplit_once('/') else {
            continue;
        };
        if !directory.ends_with("/wording") {
            continue;
        }
        let Some(language) = name.strip_suffix(".json") else {
            continue;
        };
        if language.ends_with(".forbidden") {
            continue;
        }
        let forbidden = files
            .iter()
            .find(|other| other.path == format!("{directory}/{language}.forbidden.json"))
            .cloned()
            .ok_or_else(|| format!("{}: no forbidden-words list for {language}", file.path))?;
        found.push((language.to_owned(), file.clone(), forbidden));
    }
    Ok(found)
}

#[test]
fn every_catalogue_string_is_free_of_every_forbidden_word() {
    // Arrange
    let catalogues = catalogues().unwrap();
    let mut checked = 0usize;
    let mut offending = Vec::new();

    for (language, catalogue, forbidden) in &catalogues {
        let strings = parse_catalogue(&catalogue.contents).unwrap();
        let words = parse_forbidden(&forbidden.contents).unwrap();
        assert_eq!(strings.language, *language);
        assert_eq!(words.language, *language);
        assert!(
            words.words.len() >= 10,
            "{language}: the forbidden list is too short to mean anything"
        );

        // Act
        for (key, text) in &strings.strings {
            for word in &words.words {
                if contains_forbidden_word(text, word) {
                    offending.push(format!("{language} {key}: {word:?} in {text:?}"));
                }
            }
            checked += 1;
        }
    }

    // Assert
    assert!(
        !catalogues.is_empty(),
        "no catalogue was found; this check is inert"
    );
    assert!(
        checked >= 40,
        "only {checked} strings were read; this check is inert"
    );
    assert!(
        offending.is_empty(),
        "These strings read as a verdict or as jargon:\n  {}",
        offending.join("\n  ")
    );
}

#[test]
fn every_language_has_the_same_keys_as_english() {
    // A translation that lacks a key falls back to nothing, and a sentence that is missing in one language
    // is a person left without an explanation.

    // Arrange
    let catalogues = catalogues().unwrap();
    let english = catalogues
        .iter()
        .find(|(language, _, _)| language == "en")
        .map(|(_, catalogue, _)| parse_catalogue(&catalogue.contents).unwrap())
        .unwrap();

    // Act
    let mismatched: Vec<String> = catalogues
        .iter()
        .map(|(language, catalogue, _)| (language, parse_catalogue(&catalogue.contents).unwrap()))
        .filter(|(_, catalogue)| {
            catalogue.strings.keys().collect::<Vec<_>>() != english.strings.keys().collect::<Vec<_>>()
        })
        .map(|(language, _)| language.clone())
        .collect();

    // Assert
    assert!(
        mismatched.is_empty(),
        "these languages differ from English in their keys: {mismatched:?}"
    );
}

#[test]
fn the_word_check_is_whole_word_and_case_insensitive() {
    // Act and assert
    assert!(contains_forbidden_word("This recording is VERIFIED.", "verified"));
    assert!(!contains_forbidden_word("the verifier page", "verified"));
    assert!(!contains_forbidden_word(
        "says nothing about its authenticity",
        "authentic"
    ));
    assert!(contains_forbidden_word("an original recording", "original"));
    assert!(!contains_forbidden_word("originally recorded", "original"));
}
