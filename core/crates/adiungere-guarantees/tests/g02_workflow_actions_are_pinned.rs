//! **G02.** Every workflow step runs an action pinned to a commit, with its version still visible.
//!
//! A tag is a movable pointer. Whoever controls an action's repository can move it after review, and every
//! pipeline referencing that tag then runs different code with the token the workflow was granted. One of
//! this repository's workflows can write labels, and a later one will publish releases.
//!
//! A commit cannot be moved. The version stays in a trailing comment, which is what a reader needs and what
//! the update bot recognises, so pinning costs nothing in maintenance.
//!
//! Every function below is pure and takes the files it judges. Reading the repository happens in the tests, so
//! nothing here hides an input, and each rule can be exercised against a sample that must fail it.

use adiungere_guarantees::{TextFile, workflow_files};

/// Every line declaring an action, as `file:line` and the line itself.
fn action_lines(files: &[TextFile]) -> Vec<(String, String)> {
    let mut lines = Vec::new();

    for file in files {
        for (number, text) in file.contents.lines().enumerate() {
            if text.contains("uses:") && !text.trim_start().starts_with('#') {
                lines.push((format!("{}:{}", file.path, number + 1), text.to_owned()));
            }
        }
    }

    lines
}

fn reference_of(line: &str) -> Option<&str> {
    line.split_once("uses:")?.1.split_whitespace().next()
}

fn is_pinned_to_a_commit(reference: &str) -> bool {
    reference.rsplit_once('@').is_some_and(|(_, revision)| {
        revision.len() == 40 && revision.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

#[test]
fn every_action_reference_names_a_commit() {
    // Arrange
    let files = workflow_files().unwrap();
    let lines = action_lines(&files);

    // Act
    let unpinned: Vec<String> = lines
        .iter()
        .filter_map(|(where_, line)| {
            let reference = reference_of(line)?;
            (!is_pinned_to_a_commit(reference)).then(|| format!("{where_} -> {reference}"))
        })
        .collect();

    // Assert
    assert!(
        unpinned.is_empty(),
        "These workflow steps reference an action by a tag, which whoever owns that repository can move \
         after review. Pin the commit and keep the version in a trailing comment:\n  {}",
        unpinned.join("\n  ")
    );
}

#[test]
fn every_pinned_action_keeps_its_version_visible() {
    // A bare commit is unreadable, and it stops a reader and an update bot from knowing what it stands for.

    // Arrange
    let files = workflow_files().unwrap();
    let lines = action_lines(&files);

    // Act
    let undocumented: Vec<String> = lines
        .iter()
        .filter(|(_, line)| !line.contains('#'))
        .map(|(where_, line)| format!("{where_} -> {}", line.trim()))
        .collect();

    // Assert
    assert!(
        undocumented.is_empty(),
        "These pinned actions carry no version comment:\n  {}",
        undocumented.join("\n  ")
    );
}

#[test]
fn the_check_detects_a_moving_reference() {
    // Arrange
    let moving = "      - uses: actions/checkout@v7";
    let pinned = "      - uses: actions/checkout@3d3c42e5aac5ba805825da76410c181273ba90b1 # v7.0.1";

    // Act
    let moving_verdict = reference_of(moving).map(is_pinned_to_a_commit);
    let pinned_verdict = reference_of(pinned).map(is_pinned_to_a_commit);

    // Assert
    assert_eq!(moving_verdict, Some(false), "A tag reference must be caught.");
    assert_eq!(pinned_verdict, Some(true), "A commit reference must be accepted.");
}

#[test]
fn the_scan_reads_real_workflows() {
    // Without this, a change to the workflow directory would make both checks pass by finding nothing.

    // Act
    let files = workflow_files().unwrap();
    let lines = action_lines(&files);

    // Assert
    assert!(
        !lines.is_empty(),
        "No action reference was found; the checks above are inert."
    );
}
