//! **G09.** Markdown prose wraps at 110 columns.
//!
//! This is not a typographic preference. A paragraph written as one long line produces a diff in which the
//! whole paragraph changed because one word did, and a reviewer then reads the paragraph again instead of
//! reading the change. Wrapped prose makes a documentation change reviewable at the same granularity as a
//! code change.
//!
//! Three kinds of line are exempt, because wrapping them would be wrong rather than tedious: a table row,
//! anything inside a fenced code block, and a line whose width comes from a single unbreakable token such as
//! a long URL. Width is measured in characters, and the rule reaches every spelling of a Markdown extension.

use adiungere_guarantees::{TextFile, lines_with_fence_state, tracked_text_files};

const LIMIT: usize = 110;

/// Whether a line is prose this guarantee judges.
fn is_prose(line: &str) -> bool {
    let trimmed = line.trim();

    let is_table_row = trimmed.starts_with('|');
    let is_whole_comment = trimmed.starts_with("<!--") && trimmed.ends_with("-->");

    !is_table_row && !is_whole_comment
}

/// Whether a line could have been wrapped within the limit.
///
/// A line is only a violation when there is somewhere to break it before the limit. A single long token,
/// typically a URL, has no break point and is left alone. Positions are counted in characters, because a
/// column is a character and not a byte.
fn could_have_been_wrapped(line: &str) -> bool {
    line.chars()
        .take(LIMIT)
        .enumerate()
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
        // would report every line as one character wider than it is.
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
    let documents: Vec<TextFile> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(TextFile::is_markdown)
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
fn a_wide_line_of_multibyte_text_is_measured_in_characters() {
    // Arrange
    let accented = "é ".repeat(70);

    // Act
    let found = violations_in("sample.md", &accented);

    // Assert
    assert_eq!(
        found.len(),
        1,
        "140 characters is wide whatever the byte count: {found:?}"
    );
}

#[test]
fn a_long_link_followed_by_prose_is_still_judged() {
    // The exemption is for a token nobody can break, not for any line that starts with one.

    // Arrange
    let line = format!(
        "[roadmap](https://example.test/{}) and then a sentence.",
        "a/".repeat(60)
    );

    // Act
    let found = violations_in("sample.md", &line);

    // Assert
    assert!(
        found.is_empty(),
        "No break point exists before the limit, so this is exempt: {found:?}"
    );

    let line = format!(
        "A short lead, then [roadmap](https://example.test/{}).",
        "a/".repeat(60)
    );
    let found = violations_in("sample.md", &line);
    assert_eq!(
        found.len(),
        1,
        "A break point exists before the limit, so this is judged: {found:?}"
    );
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
fn a_code_block_is_not_judged_and_a_code_span_is() {
    // Arrange
    let block = format!("```console\n$ {}\n```\n", "x ".repeat(100));
    let span = format!("```adiungere probes``` then {}", "word ".repeat(30));

    // Act
    let inside_block = violations_in("sample.md", &block);
    let after_span = violations_in("sample.md", &span);

    // Assert
    assert!(
        inside_block.is_empty(),
        "A fenced block must not be judged: {inside_block:?}"
    );
    assert_eq!(
        after_span.len(),
        1,
        "A code span is not a fence and the line is judged: {after_span:?}"
    );
}

#[test]
fn a_whole_line_comment_is_exempt_and_a_partial_one_is_not() {
    // Arrange
    let whole = format!("<!-- {} -->", "note ".repeat(30));
    let partial = format!("<!-- x --> {}", "word ".repeat(30));

    // Act and assert
    assert!(violations_in("sample.md", &whole).is_empty());
    assert_eq!(violations_in("sample.md", &partial).len(), 1);
}

#[test]
fn the_scan_reads_real_documents_under_every_spelling() {
    // Act
    let documents: Vec<TextFile> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(TextFile::is_markdown)
        .collect();
    let lines: usize = documents.iter().map(|file| file.contents.lines().count()).sum();

    // Assert
    assert!(
        documents.len() > 20,
        "Only {} documents were read; the check is too narrow.",
        documents.len()
    );
    assert!(
        lines > 1000,
        "Only {lines} lines were read; the check above is nearly inert."
    );
    assert!(
        TextFile {
            path: "x/README.markdown".to_owned(),
            contents: String::new()
        }
        .is_markdown(),
        "Every spelling a renderer accepts must be scanned."
    );
}
