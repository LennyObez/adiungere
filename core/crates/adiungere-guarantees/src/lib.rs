//! What the guarantee tests need in order to look at the repository.
//!
//! Every guarantee in this crate scans the repository, and every scan has the same two ways of being wrong.
//! It can read nothing and report success, or it can read a list written by hand and miss the very thing it
//! was meant to catch. Both are answered here: the file list always comes from git, and every scanning
//! guarantee carries a companion test proving the scan reached real files.
//!
//! Git is asked through an argument vector, never through a shell. Nothing in this crate builds a command
//! line out of a string.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// A tracked file and its contents, decoded as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextFile {
    /// The path, relative to the repository root, with forward slashes.
    pub path: String,
    /// The contents, decoded as UTF-8.
    pub contents: String,
}

impl TextFile {
    /// The file's extension, without the dot, in lower case.
    #[must_use]
    pub fn extension(&self) -> Option<String> {
        Path::new(&self.path)
            .extension()
            .and_then(OsStr::to_str)
            .map(str::to_lowercase)
    }

    /// Whether the file's path ends with this extension.
    #[must_use]
    pub fn has_extension(&self, wanted: &str) -> bool {
        self.extension().is_some_and(|found| found == wanted)
    }
}

/// Why the repository could not be read.
#[derive(Debug)]
pub enum Error {
    /// Git could not be started at all.
    GitUnavailable(std::io::Error),
    /// Git ran and refused.
    GitRefused {
        /// What git printed on its error stream.
        message: String,
    },
    /// Git listed nothing, which means the scan that follows would prove nothing.
    NothingTracked,
}

impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::GitUnavailable(cause) => {
                write!(
                    formatter,
                    "could not run git, so no guarantee here proves anything: {cause}"
                )
            },
            Self::GitRefused { message } => {
                write!(formatter, "git refused to list tracked files: {message}")
            },
            Self::NothingTracked => formatter.write_str(
                "git listed no tracked file, so every scan below would report success by reading nothing",
            ),
        }
    }
}

impl std::error::Error for Error {}

/// The root of the repository, derived from this crate's own location.
///
/// The crate sits three directories below the root, which is a fact of the layout rather than a guess about
/// the working directory a test happens to run in.
#[must_use]
pub fn repository_root() -> PathBuf {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));

    manifest.ancestors().nth(3).unwrap_or(manifest).to_path_buf()
}

/// Every path git tracks, relative to the repository root.
///
/// # Errors
///
/// Returns an error when git cannot be run, refuses, or lists nothing.
pub fn tracked_paths() -> Result<Vec<String>, Error> {
    let root = repository_root();

    let output = Command::new("git")
        .arg("-C")
        .arg(&root)
        .arg("ls-files")
        .arg("-z")
        .output()
        .map_err(Error::GitUnavailable)?;

    if !output.status.success() {
        return Err(Error::GitRefused {
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }

    let listing = String::from_utf8_lossy(&output.stdout);
    let paths: Vec<String> = listing
        .split('\0')
        .filter(|path| !path.is_empty())
        .map(str::to_owned)
        .collect();

    if paths.is_empty() {
        return Err(Error::NothingTracked);
    }

    Ok(paths)
}

/// Every tracked file whose contents decode as UTF-8, with those contents.
///
/// Files that do not decode are skipped rather than reported: a byte sequence inside a compiled asset that
/// happens to look like a forbidden pattern is noise, and every rule in this crate is about what a reader
/// can read.
///
/// # Errors
///
/// Returns an error when the file list cannot be obtained, or when nothing readable was found.
pub fn tracked_text_files() -> Result<Vec<TextFile>, Error> {
    let root = repository_root();
    let mut files = Vec::new();

    for path in tracked_paths()? {
        let full = root.join(&path);

        if let Ok(contents) = std::fs::read(&full)
            && let Ok(text) = String::from_utf8(contents)
        {
            files.push(TextFile { path, contents: text });
        }
    }

    if files.is_empty() {
        return Err(Error::NothingTracked);
    }

    Ok(files)
}

/// Every tracked file under a directory, with its contents.
///
/// # Errors
///
/// Returns an error when the repository cannot be read.
pub fn tracked_text_files_under(prefix: &str) -> Result<Vec<TextFile>, Error> {
    Ok(tracked_text_files()?
        .into_iter()
        .filter(|file| file.path.starts_with(prefix))
        .collect())
}

/// The workflow files, which several guarantees inspect.
///
/// # Errors
///
/// Returns an error when the repository cannot be read.
pub fn workflow_files() -> Result<Vec<TextFile>, Error> {
    Ok(tracked_text_files_under(".github/workflows/")?
        .into_iter()
        .filter(|file| file.has_extension("yml") || file.has_extension("yaml"))
        .collect())
}

/// Reads one tracked file, whether or not git lists it as text.
///
/// # Errors
///
/// Returns an error when the file cannot be read as UTF-8.
pub fn read_at(relative: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(repository_root().join(relative))
}

/// Returns Rust source with its comments, string literals and character literals removed.
///
/// A guarantee that searches source for a construct has to search the code and not the prose about the code.
/// Without this, the file explaining why there is no unsafe code anywhere would be the file reported for
/// containing it, and the usual fix for that is to exempt the guarantee's own file, which ends the guarantee.
///
/// Removed spans are replaced by a space so that token boundaries survive.
#[must_use]
pub fn rust_code_only(source: &str) -> String {
    let mut scanner = Scanner {
        characters: source.chars().collect(),
        index: 0,
        output: String::with_capacity(source.len()),
    };

    scanner.run();
    scanner.output
}

/// Where the scanner is in the source it is reading.
#[derive(Debug, PartialEq, Eq)]
enum Span {
    Code,
    LineComment,
    BlockComment,
    Text { raw: bool, hashes: usize },
    Character,
}

/// A single pass over Rust source, keeping the code and dropping everything written about it.
#[derive(Debug)]
struct Scanner {
    characters: Vec<char>,
    index: usize,
    output: String,
}

impl Scanner {
    fn run(&mut self) {
        let mut span = Span::Code;

        while self.index < self.characters.len() {
            span = match span {
                Span::Code => self.in_code(),
                Span::LineComment => self.in_line_comment(),
                Span::BlockComment => self.in_block_comment(),
                Span::Text { raw, hashes } => self.in_text(raw, hashes),
                Span::Character => self.in_character(),
            };
        }
    }

    fn at(&self, offset: usize) -> Option<char> {
        self.characters.get(self.index + offset).copied()
    }

    fn in_code(&mut self) -> Span {
        let current = self.at(0).unwrap_or(' ');
        let next = self.at(1);

        if current == '/' && next == Some('/') {
            self.index += 2;
            self.output.push(' ');
            return Span::LineComment;
        }

        if current == '/' && next == Some('*') {
            self.index += 2;
            self.output.push(' ');
            return Span::BlockComment;
        }

        if current == 'r'
            && matches!(next, Some('"' | '#'))
            && let Some(hashes) = self.raw_text_opening()
        {
            self.index += 2 + hashes;
            self.output.push(' ');
            return Span::Text { raw: true, hashes };
        }

        if current == '"' {
            self.index += 1;
            self.output.push(' ');
            return Span::Text {
                raw: false,
                hashes: 0,
            };
        }

        if current == '\'' && next.is_some_and(|following| following != ' ') {
            self.index += 1;
            self.output.push(' ');
            return Span::Character;
        }

        self.index += 1;
        self.output.push(current);

        Span::Code
    }

    /// How many hashes open a raw text span starting at the current `r`, if one does.
    fn raw_text_opening(&self) -> Option<usize> {
        let mut hashes = 0;

        while self.at(1 + hashes) == Some('#') {
            hashes += 1;
        }

        (self.at(1 + hashes) == Some('"')).then_some(hashes)
    }

    fn in_line_comment(&mut self) -> Span {
        let ends = self.at(0) == Some('\n');

        if ends {
            self.output.push('\n');
        }

        self.index += 1;

        if ends { Span::Code } else { Span::LineComment }
    }

    fn in_block_comment(&mut self) -> Span {
        if self.at(0) == Some('*') && self.at(1) == Some('/') {
            self.index += 2;
            return Span::Code;
        }

        self.keep_newline_and_advance();

        Span::BlockComment
    }

    fn in_text(&mut self, raw: bool, hashes: usize) -> Span {
        let current = self.at(0);

        if !raw && current == Some('\\') {
            self.index += 2;
            return Span::Text { raw, hashes };
        }

        if current == Some('"') {
            let closed = (0..hashes).all(|offset| self.at(1 + offset) == Some('#'));

            if closed {
                self.index += 1 + hashes;
                return Span::Code;
            }

            self.index += 1;
            return Span::Text { raw, hashes };
        }

        self.keep_newline_and_advance();

        Span::Text { raw, hashes }
    }

    fn in_character(&mut self) -> Span {
        if self.at(0) == Some('\\') {
            self.index += 2;
            return Span::Character;
        }

        let ends = self.at(0) == Some('\'');
        self.index += 1;

        if ends { Span::Code } else { Span::Character }
    }

    /// Advances one character, keeping a newline so that line numbers and fences survive.
    fn keep_newline_and_advance(&mut self) {
        if self.at(0) == Some('\n') {
            self.output.push('\n');
        }

        self.index += 1;
    }
}

/// Splits a document into its lines, marking the ones inside a fenced code block.
///
/// Several guarantees are about prose and must not judge a code sample, so this is shared rather than
/// written four times slightly differently.
#[must_use]
pub fn lines_with_fence_state(source: &str) -> Vec<(usize, &str, bool)> {
    let mut inside = false;

    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let is_fence = line.trim_start().starts_with("```");

            if is_fence {
                inside = !inside;
            }

            (index + 1, line, inside || is_fence)
        })
        .collect()
}
