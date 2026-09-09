//! **G09.** Markdown prose wraps at 110 columns.
//!
//! This is not a typographic preference. A paragraph written as one long line produces a diff in which the
//! whole paragraph changed because one word did, and a reviewer then reads the paragraph again instead of
//! reading the change. Wrapped prose makes a documentation change reviewable at the same granularity as a
//! code change.
//!
//! Three kinds of line are exempt, because wrapping them would be wrong rather than tedious: a table row,
//! anything inside a fenced code block, and a line whose width comes from a single unbreakable token such as
//! a long URL.

use adiungere_guarantees::{lines_with_fence_state, tracked_text_files};

const LIMIT: usize = 110;

/// Whether a line is prose this guarantee judges.
fn is_prose(line: &str) -> bool {
    let trimmed = line.trim_start();

    !trimmed.starts_with('|') && !trimmed.starts_with("<!--")
}

/// Whether a line could have been wrapped within the limit.
///
/// A line is only a violation when there is somewhere to break it. A single long token, typically a URL, has
/// no break point and is left alone.
fn could_have_been_wrapped(line: &str) -> bool {
    line.char_indices()
        .take_while(|(index, _)| *index < LIMIT)
        .skip(1)
        .any(|(_, character)| character.is_whitespace())
}

fn violations_in(path: &str, source: &str) -> Vec<String> {
    let mut found = Vec::new();

    for (number, raw, inside_fence) in lines_with_fence_state(source) {
        if inside_fence || !is_prose(raw) {
            continue;
        }

        // A carriage return is not a column. Without trimming it, a checkout that kept the other line ending
        // would report every line as one character wider than it is, and the guarantee would fail on a
        // working tree nobody had edited.
        let line = raw.trim_end();
        let width = line.chars().count();

        if width > LIMIT && could_have_been_wrapped(line) {
            found.push(format!("{path}:{number} is {width} columns wide"));
        }
    }

    found
}

#[test]
fn every_markdown_document_wraps_its_prose() {
    // Arrange
    let documents: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("md"))
        .collect();

    // Act
    let violations: Vec<String> = documents
        .iter()
        .flat_map(|file| violations_in(&file.path, &file.contents))
        .collect();

    // Assert
    assert!(
        violations.is_empty(),
        "These lines are wider than {LIMIT} columns, which makes a one-word change look like a whole \
         paragraph changing:\n  {}",
        violations.join("\n  ")
    );
}

#[test]
fn the_check_detects_a_long_paragraph() {
    // Arrange
    let long = "word ".repeat(40);

    // Act
    let found = violations_in("sample.md", &long);

    // Assert
    assert_eq!(found.len(), 1, "A long paragraph must be caught: {found:?}");
}

#[test]
fn a_table_row_is_not_judged() {
    // Tables are data. Wrapping a row would break the table.

    // Arrange
    let row = format!("| {} | {} |", "a".repeat(60), "b".repeat(60));

    // Act
    let found = violations_in("sample.md", &row);

    // Assert
    assert!(found.is_empty(), "A table row must not be judged: {found:?}");
}

#[test]
fn a_code_block_is_not_judged() {
    // Arrange
    let block = format!("```console\n$ {}\n```\n", "x".repeat(200));

    // Act
    let found = violations_in("sample.md", &block);

    // Assert
    assert!(found.is_empty(), "A fenced block must not be judged: {found:?}");
}

#[test]
fn an_unbreakable_token_is_not_judged() {
    // A long URL has no break point. Demanding one would make the guarantee unsatisfiable.

    // Arrange
    let link = format!("https://example.test/{}", "segment/".repeat(30));

    // Act
    let found = violations_in("sample.md", &link);

    // Assert
    assert!(
        found.is_empty(),
        "An unbreakable token must not be judged: {found:?}"
    );
}

#[test]
fn the_scan_reads_real_documents() {
    // Act
    let documents: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("md"))
        .collect();
    let lines: usize = documents.iter().map(|file| file.contents.lines().count()).sum();

    // Assert
    assert!(
        documents.len() > 10,
        "Only {} documents were read; the check is too narrow.",
        documents.len()
    );
    assert!(
        lines > 500,
        "Only {lines} lines were read; the check above is nearly inert."
    );
}
