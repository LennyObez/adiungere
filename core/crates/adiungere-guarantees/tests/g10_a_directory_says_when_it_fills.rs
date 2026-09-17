//! **G10.** A directory that is empty until a milestone says so, and a directory that says so is empty.
//!
//! Several of this repository's directories exist and hold nothing but a README, because the layout is the
//! shape of the whole product rather than the shape of what is built so far. That is honest only while each of
//! them says which milestone fills it. An empty directory with no explanation is a promise nobody made.
//!
//! The reverse matters as much. A README still claiming to be empty in a directory that now holds code is a
//! document contradicting the tree it sits in, and it is the kind of thing nobody notices for a year.
//!
//! A README is recognised under any casing and any Markdown extension a renderer shows, so that renaming one
//! does not take its directory out of the rule.

use adiungere_guarantees::{TextFile, is_markdown_path, read_at, tracked_paths, tracked_text_files};
use std::collections::BTreeMap;

/// The sentence a placeholder README has to contain.
const MARKER: &str = "Empty until M";

/// Whether a path is a README that a renderer would show at the top of its directory.
fn is_a_readme(path: &str) -> bool {
    let name = path.rsplit('/').next().unwrap_or(path);
    let stem = name.rsplit_once('.').map_or(name, |(stem, _)| stem);

    stem.eq_ignore_ascii_case("readme") && is_markdown_path(path)
}

/// Every tracked path, grouped by each directory it sits under, at every depth.
fn paths_by_directory(paths: &[String]) -> BTreeMap<String, Vec<String>> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for path in paths {
        let segments: Vec<&str> = path.split('/').collect();

        for depth in 1..segments.len() {
            let Some(ancestor) = segments.get(..depth) else {
                continue;
            };

            grouped.entry(ancestor.join("/")).or_default().push(path.clone());
        }
    }

    grouped
}

/// Directories whose only tracked path, at any depth beneath them, is their own README.
fn placeholder_directories(paths: &[String]) -> Vec<String> {
    paths_by_directory(paths)
        .into_iter()
        .filter(|(directory, held)| {
            held.len() == 1
                && held.first().is_some_and(|only| {
                    is_a_readme(only)
                        && only
                            .rsplit_once('/')
                            .is_some_and(|(parent, _)| parent == directory)
                })
        })
        .map(|(directory, _)| directory)
        .collect()
}

/// The milestone a placeholder README names, if it names one.
fn milestone_named_in(contents: &str) -> Option<String> {
    let after = contents.split_once(MARKER).map(|(_, rest)| rest)?;
    let digits: String = after.chars().take_while(char::is_ascii_digit).collect();

    (!digits.is_empty()).then_some(digits)
}

fn readmes_claiming_to_be_empty(files: &[TextFile]) -> Vec<&TextFile> {
    files
        .iter()
        .filter(|file| is_a_readme(&file.path) && file.contents.contains(MARKER))
        .collect()
}

#[test]
fn every_empty_directory_says_which_milestone_fills_it() {
    // Arrange
    let paths = tracked_paths().unwrap();
    let files = tracked_text_files().unwrap();
    let placeholders = placeholder_directories(&paths);

    // Act
    let silent: Vec<&String> = placeholders
        .iter()
        .filter(|directory| {
            !files.iter().any(|file| {
                is_a_readme(&file.path)
                    && file
                        .path
                        .rsplit_once('/')
                        .is_some_and(|(parent, _)| parent == *directory)
                    && file.contents.contains(MARKER)
            })
        })
        .collect();

    // Assert
    assert!(
        silent.is_empty(),
        "These directories hold nothing but a README, and the README does not say which milestone fills \
         them:\n  {}",
        silent
            .iter()
            .map(|directory| directory.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn no_directory_claims_to_be_empty_while_holding_something() {
    // Arrange
    let paths = tracked_paths().unwrap();
    let files = tracked_text_files().unwrap();
    let placeholders = placeholder_directories(&paths);

    // Act
    let contradictory: Vec<String> = readmes_claiming_to_be_empty(&files)
        .iter()
        .filter_map(|file| file.path.rsplit_once('/').map(|(parent, _)| parent.to_owned()))
        .filter(|directory| !placeholders.contains(directory))
        .collect();

    // Assert
    assert!(
        contradictory.is_empty(),
        "These READMEs still say their directory is empty, and it is not. Rewrite them to describe what is \
         there:\n  {}",
        contradictory.join("\n  ")
    );
}

#[test]
fn every_placeholder_names_a_milestone_that_exists() {
    // "Empty until M99" would satisfy the marker and say nothing true.

    // Arrange
    let roadmap = read_at("docs/roadmap.md").unwrap();
    let files = tracked_text_files().unwrap();

    // Act
    let unknown: Vec<String> = readmes_claiming_to_be_empty(&files)
        .iter()
        .filter_map(|file| {
            let named = milestone_named_in(&file.contents)?;

            (!roadmap.contains(&format!("## M{named}:"))).then(|| format!("{} names M{named}", file.path))
        })
        .collect();

    // Assert
    assert!(
        unknown.is_empty(),
        "These READMEs name a milestone the roadmap does not have:\n  {}",
        unknown.join("\n  ")
    );
}

#[test]
fn every_placeholder_names_a_milestone_at_all() {
    // The marker without a number would pass the check above by returning nothing to compare.

    // Arrange
    let files = tracked_text_files().unwrap();

    // Act
    let vague: Vec<String> = readmes_claiming_to_be_empty(&files)
        .iter()
        .filter(|file| milestone_named_in(&file.contents).is_none())
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        vague.is_empty(),
        "These READMEs say they are empty until a milestone and name none:\n  {}",
        vague.join("\n  ")
    );
}

#[test]
fn the_check_detects_a_populated_directory_still_claiming_to_be_empty() {
    // The reverse rule, exercised on a sample: a README with the marker in a directory holding code.

    // Arrange
    let paths = vec!["design/README.md".to_owned(), "design/tokens.json".to_owned()];
    let files = vec![TextFile {
        path: "design/README.md".to_owned(),
        contents: "> **Empty until M4.**".to_owned(),
    }];

    // Act
    let placeholders = placeholder_directories(&paths);
    let claiming: Vec<&TextFile> = readmes_claiming_to_be_empty(&files);

    // Assert
    assert!(
        !placeholders.contains(&"design".to_owned()),
        "A directory holding code is not a placeholder."
    );
    assert_eq!(
        claiming.len(),
        1,
        "The README still claims to be empty and must be reported."
    );
}

#[test]
fn a_readme_is_recognised_under_any_casing_and_extension() {
    // Act and assert
    assert!(is_a_readme("infra/README.md"));
    assert!(is_a_readme("infra/readme.md"));
    assert!(is_a_readme("infra/Readme.markdown"));
    assert!(!is_a_readme("infra/README.txt"));
    assert!(!is_a_readme("infra/notes.md"));
}

#[test]
fn the_check_finds_the_placeholders_that_exist_today() {
    // Without this, a change to how directories are grouped would let both checks pass by finding nothing.

    // Arrange
    let paths = tracked_paths().unwrap();

    // Act
    let placeholders = placeholder_directories(&paths);

    // Assert
    assert!(
        placeholders.len() >= 5,
        "Only {} placeholder directories were found; the checks above are nearly inert: {placeholders:?}",
        placeholders.len()
    );
    assert!(
        placeholders.contains(&"design".to_owned()),
        "The design directory is a placeholder today."
    );
    assert!(
        !placeholders.contains(&"core".to_owned()),
        "The Rust workspace is not a placeholder."
    );
}
