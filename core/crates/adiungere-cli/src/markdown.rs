//! The little Markdown the two document parsers need to agree on.
//!
//! Both the evidence register and the roadmap are Markdown files read as data. They have to recognise the
//! same three things in the same way: a fenced code block, a heading, and an HTML comment. When they did not,
//! a fenced example in one file was ignored and the same example in the other file was read as a real
//! reference, and the two documents disagreed about a probe nobody had written.
//!
//! Only the rules that matter here are implemented, and they follow the common specification where it has
//! a rule:
//! a fence is three or more backticks or tildes, an info string after a backtick fence may not contain a
//! backtick, and a fence closes only on a fence of the same character at least as long as the one that
//! opened it.

/// What a line is, once fences are taken into account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineKind {
    /// Ordinary text outside any fenced block.
    Outside,
    /// The line that opens a fenced block.
    Opens,
    /// A line inside a fenced block.
    Inside,
    /// The line that closes a fenced block.
    Closes,
}

/// Tracks fenced code blocks across the lines of a document.
#[derive(Debug, Default)]
pub struct FenceTracker {
    open: Option<Fence>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fence {
    character: char,
    length: usize,
    line: usize,
}

impl FenceTracker {
    /// A tracker outside any fence.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Classifies the next line of the document.
    pub fn observe(&mut self, line: &str, number: usize) -> LineKind {
        let trimmed = line.trim_start();
        let indent = line.len().saturating_sub(trimmed.len());

        match self.open {
            None => {
                if indent <= 3
                    && let Some(fence) = opening_fence(trimmed, number)
                {
                    self.open = Some(fence);
                    return LineKind::Opens;
                }

                LineKind::Outside
            },
            Some(fence) => {
                if indent <= 3 && closes(trimmed, fence) {
                    self.open = None;
                    return LineKind::Closes;
                }

                LineKind::Inside
            },
        }
    }

    /// The line on which the still-open fence started, if the document ended inside one.
    #[must_use]
    pub fn unterminated(&self) -> Option<usize> {
        self.open.map(|fence| fence.line)
    }
}

fn opening_fence(trimmed: &str, number: usize) -> Option<Fence> {
    let character = trimmed.chars().next()?;

    if character != '`' && character != '~' {
        return None;
    }

    let length = trimmed.chars().take_while(|found| *found == character).count();

    if length < 3 {
        return None;
    }

    let info = trimmed.get(length..).unwrap_or("");

    if character == '`' && info.contains('`') {
        return None;
    }

    Some(Fence {
        character,
        length,
        line: number,
    })
}

fn closes(trimmed: &str, fence: Fence) -> bool {
    let length = trimmed
        .chars()
        .take_while(|found| *found == fence.character)
        .count();

    length >= fence.length && trimmed.get(length..).is_some_and(|rest| rest.trim().is_empty())
}

/// A heading's level and text, when a line is one.
///
/// A heading is one to six `#` followed by at least one space, or by nothing. `#hashtag` is not a heading.
#[must_use]
pub fn heading(line: &str) -> Option<(usize, &str)> {
    let level = line.chars().take_while(|character| *character == '#').count();

    if level == 0 || level > 6 {
        return None;
    }

    let rest = line.get(level..)?;

    if rest.is_empty() {
        return Some((level, ""));
    }

    let text = rest.strip_prefix(' ')?;

    Some((level, text.trim()))
}

/// The text with every HTML comment removed, including comments spanning several lines.
///
/// A comment is where a document hides a note from its reader, and a note hidden from the reader must not be
/// read as a statement the document makes.
#[must_use]
pub fn without_html_comments(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find("<!--") {
        output.push_str(rest.get(..start).unwrap_or(""));

        let after = rest.get(start + 4..).unwrap_or("");

        match after.find("-->") {
            Some(end) => rest = after.get(end + 3..).unwrap_or(""),
            None => return output,
        }
    }

    output.push_str(rest);
    output
}

#[cfg(test)]
mod tests {
    use super::{FenceTracker, LineKind, heading, without_html_comments};

    fn classify(lines: &[&str]) -> Vec<LineKind> {
        let mut tracker = FenceTracker::new();

        lines
            .iter()
            .enumerate()
            .map(|(index, line)| tracker.observe(line, index + 1))
            .collect()
    }

    #[test]
    fn a_backtick_fence_opens_and_closes() {
        // Act
        let kinds = classify(&["text", "```rust", "code", "```", "after"]);

        // Assert
        assert_eq!(
            kinds,
            [
                LineKind::Outside,
                LineKind::Opens,
                LineKind::Inside,
                LineKind::Closes,
                LineKind::Outside
            ]
        );
    }

    #[test]
    fn a_code_span_with_backticks_in_its_info_string_is_not_a_fence() {
        // A line such as "```adiungere probes``` reads the register." is prose with a code span, and treating
        // it as a fence would swallow the rest of the document.

        // Act
        let kinds = classify(&["```adiungere probes``` reads the register.", "next"]);

        // Assert
        assert_eq!(kinds, [LineKind::Outside, LineKind::Outside]);
    }

    #[test]
    fn a_longer_fence_contains_a_shorter_one() {
        // Act
        let kinds = classify(&["````markdown", "```", "inner", "```", "````", "out"]);

        // Assert
        assert_eq!(
            kinds,
            [
                LineKind::Opens,
                LineKind::Inside,
                LineKind::Inside,
                LineKind::Inside,
                LineKind::Closes,
                LineKind::Outside
            ]
        );
    }

    #[test]
    fn a_tilde_fence_is_a_fence_and_does_not_close_a_backtick_one() {
        // Act
        let kinds = classify(&["~~~", "~~~", "```", "~~~", "```"]);

        // Assert
        assert_eq!(
            kinds,
            [
                LineKind::Opens,
                LineKind::Closes,
                LineKind::Opens,
                LineKind::Inside,
                LineKind::Closes
            ]
        );
    }

    #[test]
    fn an_unterminated_fence_is_reported_with_its_line() {
        // Arrange
        let mut tracker = FenceTracker::new();

        // Act
        tracker.observe("prose", 1);
        tracker.observe("```", 2);
        tracker.observe("never closed", 3);

        // Assert
        assert_eq!(tracker.unterminated(), Some(2));
    }

    #[test]
    fn a_heading_needs_a_space_after_its_marks() {
        // Act and assert
        assert_eq!(heading("## P01 A title"), Some((2, "P01 A title")));
        assert_eq!(heading("# Top"), Some((1, "Top")));
        assert_eq!(heading("#hashtag"), None);
        assert_eq!(heading("####### seven"), None);
        assert_eq!(heading("##"), Some((2, "")));
        assert_eq!(heading("plain"), None);
    }

    #[test]
    fn html_comments_are_removed_across_lines() {
        // Act
        let cleaned = without_html_comments("keep <!-- drop\nthis P99 --> keep too <!-- open");

        // Assert
        assert_eq!(cleaned, "keep  keep too ");
    }
}
