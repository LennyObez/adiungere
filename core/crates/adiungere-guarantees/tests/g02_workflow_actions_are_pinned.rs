//! **G02.** Every action a pipeline runs is pinned to a commit, with its version still visible.
//!
//! A tag is a movable pointer. Whoever controls an action's repository can move it after review, and every
//! pipeline referencing that tag then runs different code with the token the workflow was granted. One of
//! this repository's workflows can write labels, and a later one will publish releases.
//!
//! A commit cannot be moved. The version stays in a trailing comment, which is what a reader needs and what
//! the update bot recognises, so pinning costs nothing in maintenance.
//!
//! The rule reaches every place an action can be named: the workflows, and any composite action the
//! repository defines under `.github/actions`. It reads the `uses` key in each spelling YAML allows, because
//! a key written with quotes is the same key.

use adiungere_guarantees::{TextFile, tracked_text_files_under};

/// Every line declaring an action, as `file:line` and the reference it names.
fn action_references(files: &[TextFile]) -> Vec<(String, String, String)> {
    let mut found = Vec::new();

    for file in files {
        for (number, text) in file.contents.lines().enumerate() {
            if let Some(reference) = reference_of(text) {
                found.push((
                    format!("{}:{}", file.path, number + 1),
                    reference.to_owned(),
                    text.to_owned(),
                ));
            }
        }
    }

    found
}

/// The action a line names, if the line is a `uses` entry in any of the spellings YAML allows.
fn reference_of(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();

    if trimmed.starts_with('#') {
        return None;
    }

    let after_dash = trimmed.strip_prefix("- ").map_or(trimmed, str::trim_start);
    let after_key = ["uses", "\"uses\"", "'uses'"]
        .iter()
        .find_map(|key| after_dash.strip_prefix(key))?;
    let after_colon = after_key.trim_start().strip_prefix(':')?;

    after_colon
        .split_whitespace()
        .next()
        .map(|reference| reference.trim_matches(|c| c == '"' || c == '\''))
}

/// Whether a reference names an immutable revision: a 40-character commit, or an image digest.
fn is_pinned(reference: &str) -> bool {
    if let Some(image) = reference.strip_prefix("docker://") {
        return image
            .rsplit_once("@sha256:")
            .is_some_and(|(_, digest)| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()));
    }

    reference.rsplit_once('@').is_some_and(|(_, revision)| {
        revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

/// Whether a line carries a version in its trailing comment: a `v` followed by a digit, or a dotted number.
fn names_a_version(line: &str) -> bool {
    let Some((_, comment)) = line.split_once('#') else {
        return false;
    };

    comment.split_whitespace().any(|word| {
        let word = word.trim_matches(|c: char| !c.is_alphanumeric() && c != '.');
        let digits_and_dots = word.chars().filter(|c| c.is_ascii_digit() || *c == '.').count();

        (word.starts_with('v') || word.contains("-v")) && digits_and_dots >= 1
            || (word.contains('.')
                && digits_and_dots >= 3
                && word.chars().next().is_some_and(|c| c.is_ascii_digit()))
    })
}

/// The workflows and any composite action, read once per test.
fn action_files() -> Result<Vec<TextFile>, adiungere_guarantees::Error> {
    let mut files = tracked_text_files_under(".github/workflows/")?;
    files.extend(tracked_text_files_under(".github/actions/")?);

    Ok(files
        .into_iter()
        .filter(|file| file.has_extension("yml") || file.has_extension("yaml"))
        .collect())
}

#[test]
fn every_action_reference_names_an_immutable_revision() {
    // Arrange
    let files = action_files().unwrap();

    // Act
    let unpinned: Vec<String> = action_references(&files)
        .into_iter()
        .filter(|(_, reference, _)| !is_pinned(reference))
        .map(|(where_, reference, _)| format!("{where_} -> {reference}"))
        .collect();

    // Assert
    assert!(
        unpinned.is_empty(),
        "These steps reference an action by a tag, which whoever owns that repository can move after \
         review. Pin the commit and keep the version in a trailing comment:\n  {}",
        unpinned.join("\n  ")
    );
}

#[test]
fn every_pinned_action_keeps_its_version_visible() {
    // A bare commit is unreadable, and it stops a reader and an update bot from knowing what it stands for.
    // A comment that says "pinned" is not a version.

    // Arrange
    let files = action_files().unwrap();

    // Act
    let undocumented: Vec<String> = action_references(&files)
        .into_iter()
        .filter(|(_, _, line)| !names_a_version(line))
        .map(|(where_, _, line)| format!("{where_} -> {}", line.trim()))
        .collect();

    // Assert
    assert!(
        undocumented.is_empty(),
        "These pinned actions carry no version in their comment:\n  {}",
        undocumented.join("\n  ")
    );
}

#[test]
fn the_check_detects_a_moving_reference_and_accepts_a_pinned_one() {
    // Act and assert
    assert_eq!(
        reference_of("      - uses: actions/checkout@v7").map(is_pinned),
        Some(false),
        "A tag reference must be caught."
    );
    assert_eq!(
        reference_of("      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1")
            .map(is_pinned),
        Some(true),
        "A commit reference must be accepted."
    );
    assert_eq!(
        reference_of("      - uses: docker://alpine:3.20").map(is_pinned),
        Some(false),
        "An image by tag must be caught."
    );
}

#[test]
fn the_check_reads_every_spelling_of_the_key() {
    // Act and assert
    assert_eq!(
        reference_of("      - \"uses\": actions/checkout@v7"),
        Some("actions/checkout@v7")
    );
    assert_eq!(
        reference_of("        uses : 'actions/checkout@v7'"),
        Some("actions/checkout@v7")
    );
    assert_eq!(reference_of("      # uses: actions/checkout@v7"), None);
    assert_eq!(reference_of("      - name: uses nothing"), None);
}

#[test]
fn the_version_comment_check_wants_a_version() {
    // Act and assert
    assert!(names_a_version(
        "- uses: a/b@0000000000000000000000000000000000000000 # v7.0.1"
    ));
    assert!(names_a_version(
        "- uses: a/b@0000000000000000000000000000000000000000 # codeql-bundle-v2.27.0"
    ));
    assert!(!names_a_version(
        "- uses: a/b@0000000000000000000000000000000000000000 # pinned"
    ));
    assert!(!names_a_version(
        "- uses: a/b@0000000000000000000000000000000000000000"
    ));
}

#[test]
fn the_scan_reads_real_workflows() {
    // Without this, a change to the workflow directory would make both checks pass by finding nothing.

    // Act
    let references = action_references(&action_files().unwrap());

    // Assert
    assert!(
        references.len() >= 5,
        "Only {} action references were found; the checks above are nearly inert.",
        references.len()
    );
}
