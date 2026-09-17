//! What the guarantee tests need in order to look at the repository.
//!
//! Every guarantee in this crate scans the repository, and every scan has the same two ways of being wrong.
//! It can read nothing and report success, or it can read a list written by hand and miss the very thing it
//! was meant to catch. Both are answered here: the file list always comes from git, and every scanning
//! guarantee carries a companion test proving the scan reached real files.
//!
//! A third way of being wrong was found and closed: a scan that silently drops a file it cannot decode. A
//! document saved in the wrong encoding is invisible to an editor's eye and to every rule at once, so a
//! tracked file that is not declared binary and does not decode as UTF-8 is refused rather than skipped.
//!
//! Git is asked through an argument vector, never through a shell. Nothing in this crate builds a command
//! line out of a string.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Extensions that GitHub and this repository treat as Markdown.
pub const MARKDOWN_EXTENSIONS: [&str; 5] = ["md", "markdown", "mdown", "mkd", "mdwn"];

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
        extension_of(&self.path)
    }

    /// Whether the file's path ends with this extension, compared without regard to case.
    #[must_use]
    pub fn has_extension(&self, wanted: &str) -> bool {
        self.extension()
            .is_some_and(|found| found == wanted.to_lowercase())
    }

    /// Whether the file is Markdown under any of the spellings a renderer accepts.
    #[must_use]
    pub fn is_markdown(&self) -> bool {
        is_markdown_path(&self.path)
    }

    /// The file's name without its directory.
    #[must_use]
    pub fn name(&self) -> &str {
        self.path.rsplit('/').next().unwrap_or(&self.path)
    }
}

/// The extension of a path, without the dot, in lower case.
#[must_use]
pub fn extension_of(path: &str) -> Option<String> {
    Path::new(path)
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_lowercase)
}

/// Whether a path is Markdown under any of the spellings a renderer accepts.
#[must_use]
pub fn is_markdown_path(path: &str) -> bool {
    extension_of(path).is_some_and(|extension| MARKDOWN_EXTENSIONS.contains(&extension.as_str()))
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
    /// A tracked file could not be read from disk.
    Unreadable {
        /// The path, relative to the repository root.
        path: String,
        /// What the operating system said.
        cause: std::io::Error,
    },
    /// A tracked file is not declared binary and does not decode as UTF-8.
    NotText {
        /// The path, relative to the repository root.
        path: String,
    },
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
            Self::Unreadable { path, cause } => {
                write!(formatter, "{path} is tracked and cannot be read: {cause}")
            },
            Self::NotText { path } => write!(
                formatter,
                "{path} is not declared binary in .gitattributes and does not decode as UTF-8, so every \
                 scan would skip it. Fix its encoding, or declare it binary."
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

/// The extensions `.gitattributes` declares binary, in lower case and without the dot.
///
/// Read from the file rather than written here, so the one place that says what is binary stays the one
/// place. A file with one of these extensions is skipped by every text scan; a file with any other extension
/// has to decode as UTF-8.
///
/// # Errors
///
/// Returns an error when the attributes file cannot be read.
pub fn binary_extensions() -> Result<BTreeSet<String>, Error> {
    let source = read_at(".gitattributes").map_err(|cause| Error::Unreadable {
        path: ".gitattributes".to_owned(),
        cause,
    })?;

    Ok(source
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let pattern = parts.next()?;
            let declares_binary = parts.any(|attribute| attribute == "binary");

            (declares_binary && pattern.starts_with("*."))
                .then(|| pattern.trim_start_matches("*.").to_lowercase())
        })
        .collect())
}

/// Every tracked file that is not declared binary, with its contents decoded as UTF-8.
///
/// # Errors
///
/// Returns an error when the file list cannot be obtained, when a file cannot be read, or when a file that
/// is not declared binary does not decode. The last one is deliberate: the alternative is a scan that
/// quietly reads less than the repository holds.
pub fn tracked_text_files() -> Result<Vec<TextFile>, Error> {
    let root = repository_root();
    let binary = binary_extensions()?;
    let mut files = Vec::new();

    for path in tracked_paths()? {
        if extension_of(&path).is_some_and(|extension| binary.contains(&extension)) {
            continue;
        }

        let contents = std::fs::read(root.join(&path)).map_err(|cause| Error::Unreadable {
            path: path.clone(),
            cause,
        })?;

        let text = String::from_utf8(contents).map_err(|_| Error::NotText { path: path.clone() })?;

        files.push(TextFile { path, contents: text });
    }

    if files.is_empty() {
        return Err(Error::NothingTracked);
    }

    Ok(files)
}

/// Every tracked text file under a directory, with its contents.
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

/// The value of a key inside one table of the versions file, `tools/versions.toml`.
#[must_use]
pub fn pinned(table: &str, key: &str) -> Option<String> {
    let source = without_hash_comments(&read_at("tools/versions.toml").ok()?);
    let mut inside = false;
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == format!("[{table}]");
            continue;
        }
        if inside
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

/// Whether a candidate media tool sits where the versions file says the fetched one is, or on the path,
/// and reports the pinned version. The versions file decides acceptance and nothing else: no value read
/// from it becomes part of a command.
fn is_the_pinned_media_tool(candidate: &Path, tools: &Path) -> bool {
    let Some(version) = pinned("ffmpeg", "version") else {
        return false;
    };
    let Some(binary) = pinned("ffmpeg.linux-x86_64", "binary") else {
        return false;
    };
    if candidate.starts_with(tools) && candidate != tools.join(binary) {
        return false;
    }
    let Ok(output) = Command::new(candidate).arg("-version").output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .is_some_and(|first| first.contains(&version))
}

/// The pinned media tool, from the fetched tools directory or the path, at the pinned version only, or
/// the sentence to print when it is absent. A guarantee that needs it fails on that sentence rather than
/// skipping: a step that did not run is not a step that passed.
///
/// The candidates come from listing the tools directory, never from a value read out of a file.
///
/// # Errors
///
/// Returns the sentence when no candidate reports the pinned version.
pub fn pinned_media_tool() -> Result<PathBuf, String> {
    let tools = repository_root().join(".tools");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&tools)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("bin").join("ffmpeg"))
                .collect()
        })
        .unwrap_or_default();
    candidates.push(PathBuf::from("ffmpeg"));

    candidates
        .into_iter()
        .find(|candidate| is_the_pinned_media_tool(candidate, &tools))
        .ok_or_else(|| {
            "the media tool at the version pinned in tools/versions.toml was not found. Run \
             scripts/fetch-tools.sh; a guarantee that skipped it would be a guarantee that proved nothing"
                .to_owned()
        })
}

/// The probing companion of the pinned media tool, which the same archive places beside it, accepted
/// only when it reports the same pinned version: a companion found beside the tool but built from
/// something else would count packets with a different parser than the one the guarantee names.
///
/// # Errors
///
/// Returns the sentence when the media tool is absent, or when the companion beside it is absent or
/// reports another version.
pub fn pinned_media_prober() -> Result<PathBuf, String> {
    let tool = pinned_media_tool()?;
    [tool.with_file_name("ffprobe")]
        .into_iter()
        .find(|candidate| reports_the_pinned_version(candidate))
        .ok_or_else(|| {
            format!(
                "the probing companion of the media tool is absent beside {} or is not at the pinned \
                 version. Run scripts/fetch-tools.sh; a guarantee that counted packets with another parser \
                 would be comparing against the wrong oracle",
                tool.display()
            )
        })
}

/// Whether a candidate tool prints the pinned media tool version on the first line of its version report.
fn reports_the_pinned_version(candidate: &Path) -> bool {
    let Some(version) = pinned("ffmpeg", "version") else {
        return false;
    };
    let Ok(output) = Command::new(candidate).arg("-version").output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .is_some_and(|first| first.contains(&version))
}

/// Whether a candidate validator sits where the versions file says the fetched one is, or on the path,
/// and reports the pinned version.
fn is_the_pinned_validator(candidate: &Path, tools: &Path) -> bool {
    let Some(version) = pinned("c2patool", "version") else {
        return false;
    };
    let Some(binary) = pinned("c2patool.linux-x86_64", "binary") else {
        return false;
    };
    if candidate.starts_with(tools) && candidate != tools.join(binary) {
        return false;
    }
    let Ok(output) = Command::new(candidate).arg("--version").output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .is_some_and(|first| first.contains(&version))
}

/// The pinned Content Credentials validator, from the fetched tools directory or the path, at the pinned
/// version only, or the sentence to print when it is absent. It is the independent reader of what the
/// product signs, and a guarantee that needs it fails on that sentence rather than skipping.
///
/// # Errors
///
/// Returns the sentence when no candidate reports the pinned version.
pub fn pinned_validator() -> Result<PathBuf, String> {
    let tools = repository_root().join(".tools");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&tools)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("c2patool"))
                .collect()
        })
        .unwrap_or_default();
    candidates.push(PathBuf::from("c2patool"));

    candidates
        .into_iter()
        .find(|candidate| is_the_pinned_validator(candidate, &tools))
        .ok_or_else(|| {
            "the credentials validator at the version pinned in tools/versions.toml was not found. Run \
             scripts/fetch-tools.sh; a guarantee that skipped it would be a guarantee that proved nothing"
                .to_owned()
        })
}

/// What the pinned validator says about a file, as the JSON document it prints: the active manifest, the
/// manifests, the validation state and the validation results.
///
/// # Errors
///
/// Returns the validator's error stream when it refuses, or the parse error when it prints something
/// that is not one JSON document.
pub fn validator_report(validator: &Path, file: &Path) -> Result<serde_json::Value, String> {
    let output = Command::new(validator)
        .arg(file)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
}

/// The value at a path of keys inside a JSON document, or nothing when a key is absent.
#[must_use]
pub fn at<'a>(value: &'a serde_json::Value, path: &[&str]) -> Option<&'a serde_json::Value> {
    path.iter().try_fold(value, |current, key| current.get(key))
}

/// The string at a path of keys, or an empty string.
#[must_use]
pub fn text_at(value: &serde_json::Value, path: &[&str]) -> String {
    at(value, path)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

/// What the pinned validator found about a file, read out of its report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatorVerdict {
    /// The validation state the validator names.
    pub state: String,
    /// The common name of the signer's certificate.
    pub common_name: String,
    /// The codes the validator reports as failures.
    pub failures: Vec<String>,
    /// The actions the active manifest records, in order.
    pub actions: Vec<String>,
    /// The data of the assertion the product writes its facts under, when the manifest carries it.
    pub integrity: Option<serde_json::Value>,
}

/// Runs the pinned validator on a file and reads its verdict.
///
/// # Errors
///
/// Returns the validator's error stream when it refuses, or the reason the report could not be read.
pub fn validator_verdict(validator: &Path, file: &Path) -> Result<ValidatorVerdict, String> {
    let report = validator_report(validator, file)?;
    let active = text_at(&report, &["active_manifest"]);
    if active.is_empty() {
        return Err("the validator's report names no active manifest".to_owned());
    }
    let manifest = at(&report, &["manifests", &active]).ok_or("the active manifest is not in the report")?;
    let assertions = at(manifest, &["assertions"])
        .and_then(serde_json::Value::as_array)
        .cloned()
        .unwrap_or_default();
    let failures = at(&report, &["validation_results", "activeManifest", "failure"])
        .and_then(serde_json::Value::as_array)
        .map(|codes| codes.iter().map(|code| text_at(code, &["code"])).collect())
        .unwrap_or_default();
    let actions = assertions
        .iter()
        .filter(|assertion| text_at(assertion, &["label"]).starts_with("c2pa.actions"))
        .flat_map(|assertion| {
            at(assertion, &["data", "actions"])
                .and_then(serde_json::Value::as_array)
                .cloned()
                .unwrap_or_default()
        })
        .map(|action| text_at(&action, &["action"]))
        .collect();
    let integrity = assertions
        .iter()
        .find(|assertion| text_at(assertion, &["label"]) == "com.adiungere.integrity")
        .and_then(|assertion| at(assertion, &["data"]).cloned());
    Ok(ValidatorVerdict {
        state: text_at(&report, &["validation_state"]),
        common_name: text_at(manifest, &["signature_info", "common_name"]),
        failures,
        actions,
        integrity,
    })
}

/// Returns the text with every line comment and every line that is only a comment removed, for
/// configuration files that use `#`.
///
/// A rule about what a configuration declares must not be satisfied by a commented-out line that declares it,
/// nor defeated by a comment that mentions the forbidden thing.
#[must_use]
pub fn without_hash_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            let mut inside_quotes = false;
            let mut kept = String::with_capacity(line.len());

            for character in line.chars() {
                if character == '"' {
                    inside_quotes = !inside_quotes;
                }

                if character == '#' && !inside_quotes {
                    break;
                }

                kept.push(character);
            }

            kept.trim_end().to_owned()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns Rust source with its comments, string literals and character literals removed.
///
/// A guarantee that searches source for a construct has to search the code and not the prose about the code.
/// Without this, the file explaining why there is no unsafe code anywhere would be the file reported for
/// containing it, and the usual fix for that is to exempt the guarantee's own file, which ends the guarantee.
///
/// Removed spans are replaced by a space so that token boundaries survive. The lexer knows exactly as much
/// Rust as this needs: line and nested block comments, plain, raw and byte strings, character literals, and
/// the one case that looks like a character literal and is not, a lifetime such as `'static`.
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
    BlockComment { depth: usize },
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
                Span::BlockComment { depth } => self.in_block_comment(depth),
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
            return Span::BlockComment { depth: 1 };
        }

        // A byte string or raw byte string: `b"…"`, `br"…"`, `br#"…"#`. The prefix is code; the text is not.
        if current == 'b'
            && !self.preceded_by_identifier()
            && let Some((prefix, hashes, raw)) = self.text_opening(1)
        {
            self.index += 1 + prefix;
            self.output.push(' ');
            return Span::Text { raw, hashes };
        }

        if let Some((prefix, hashes, raw)) = self.text_opening(0)
            && !(current == 'r' && self.preceded_by_identifier())
        {
            self.index += prefix;
            self.output.push(' ');
            return Span::Text { raw, hashes };
        }

        if current == '\'' {
            if self.opens_character_literal() {
                self.index += 1;
                self.output.push(' ');
                return Span::Character;
            }

            // A lifetime or a label: `'static`, `'a`, `'outer:`. Code, kept as it is.
            self.index += 1;
            self.output.push(current);
            return Span::Code;
        }

        self.index += 1;
        self.output.push(current);

        Span::Code
    }

    /// Whether the character before the current one belongs to an identifier, in which case a `b` or `r`
    /// here is the tail of a name rather than a prefix.
    fn preceded_by_identifier(&self) -> bool {
        self.index
            .checked_sub(1)
            .and_then(|before| self.characters.get(before))
            .is_some_and(|previous| previous.is_alphanumeric() || *previous == '_')
    }

    /// If a plain or raw string opens at the current position plus `skip`, how many characters the opening
    /// takes, how many hashes it carries, and whether it is raw.
    fn text_opening(&self, skip: usize) -> Option<(usize, usize, bool)> {
        match self.at(skip)? {
            '"' => Some((skip + 1, 0, false)),
            'r' => {
                let mut hashes = 0;

                while self.at(skip + 1 + hashes) == Some('#') {
                    hashes += 1;
                }

                (self.at(skip + 1 + hashes) == Some('"')).then_some((skip + 2 + hashes, hashes, true))
            },
            _ => None,
        }
    }

    /// Whether the `'` at the current position opens a character literal rather than a lifetime.
    ///
    /// A character literal is `'x'`, `'\n'`, `'\u{2014}'`, `'\''`: one character or one escape, then a
    /// closing quote. A lifetime is `'` followed by an identifier and no closing quote.
    fn opens_character_literal(&self) -> bool {
        match self.at(1) {
            None => false,
            Some('\\') => true,
            Some(first) => {
                if self.at(2) == Some('\'') {
                    return true;
                }

                // `'ab'` is not valid Rust, so anything longer than one character before a quote is a
                // lifetime or a label, never a literal.
                !(first.is_alphanumeric() || first == '_')
            },
        }
    }

    fn in_line_comment(&mut self) -> Span {
        let ends = self.at(0) == Some('\n');

        if ends {
            self.output.push('\n');
        }

        self.index += 1;

        if ends { Span::Code } else { Span::LineComment }
    }

    fn in_block_comment(&mut self, depth: usize) -> Span {
        if self.at(0) == Some('/') && self.at(1) == Some('*') {
            self.index += 2;
            return Span::BlockComment { depth: depth + 1 };
        }

        if self.at(0) == Some('*') && self.at(1) == Some('/') {
            self.index += 2;

            return if depth <= 1 {
                Span::Code
            } else {
                Span::BlockComment { depth: depth - 1 }
            };
        }

        self.keep_newline_and_advance();

        Span::BlockComment { depth }
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
/// written four times slightly differently. It follows the same fence rules as the register and roadmap
/// parsers: three or more backticks or tildes open a block, an info string after a backtick fence may not
/// contain a backtick, and a block closes only on a fence of the same character at least as long.
#[must_use]
pub fn lines_with_fence_state(source: &str) -> Vec<(usize, &str, bool)> {
    let mut open: Option<(char, usize)> = None;

    source
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let trimmed = line.trim_start();
            let indent = line.len().saturating_sub(trimmed.len());
            let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~');
            let run = marker.map_or(0, |c| trimmed.chars().take_while(|found| *found == c).count());
            let rest = trimmed.get(run..).unwrap_or("");

            let inside = match (open, marker) {
                (None, Some(character)) if indent <= 3 && run >= 3 => {
                    let is_fence = character == '~' || !rest.contains('`');

                    if is_fence {
                        open = Some((character, run));
                    }

                    is_fence
                },
                (None, _) => false,
                (Some((character, length)), Some(found))
                    if indent <= 3 && found == character && run >= length && rest.trim().is_empty() =>
                {
                    open = None;
                    true
                },
                (Some(_), _) => true,
            };

            (index + 1, line, inside)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{lines_with_fence_state, rust_code_only, without_hash_comments};

    fn tokens(source: &str) -> Vec<String> {
        rust_code_only(source)
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|token| !token.is_empty())
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn comments_and_strings_are_dropped_and_code_is_kept() {
        // Arrange
        let source = "// unsafe here\nfn f() -> &str { \"unsafe\" } /* unsafe */ let x = 1;";

        // Act
        let found = tokens(source);

        // Assert
        assert_eq!(found, ["fn", "f", "str", "let", "x", "1"]);
    }

    #[test]
    fn a_lifetime_is_code_and_not_the_start_of_a_character_literal() {
        // Without this, `'static` swallowed everything up to the next quote, which in a real file was most
        // of the file, and an unsafe block after it was invisible.

        // Arrange
        let source = "fn f<'a>(x: &'a str) -> &'static str { unsafe { g(x) } }";

        // Act
        let found = tokens(source);

        // Assert
        assert!(found.contains(&"unsafe".to_owned()), "got {found:?}");
        assert!(found.contains(&"static".to_owned()), "got {found:?}");
    }

    #[test]
    fn character_literals_are_dropped_including_the_awkward_ones() {
        // Arrange
        let source = "let a = ' '; let b = '\\''; let c = '\"'; let d = '\\u{2014}'; unsafe { }";

        // Act
        let found = tokens(source);

        // Assert
        assert_eq!(found, ["let", "a", "let", "b", "let", "c", "let", "d", "unsafe"]);
    }

    #[test]
    fn raw_and_byte_strings_are_dropped() {
        // Arrange
        let source = "let a = r\"unsafe\"; let b = r#\"un\"safe\"#; let c = b\"unsafe\"; let d = br\"x\"; ok";

        // Act
        let found = tokens(source);

        // Assert
        assert_eq!(found, ["let", "a", "let", "b", "let", "c", "let", "d", "ok"]);
    }

    #[test]
    fn block_comments_nest() {
        // Arrange
        let source = "/* a /* b */ unsafe */ fn f() {}";

        // Act
        let found = tokens(source);

        // Assert
        assert_eq!(found, ["fn", "f"]);
    }

    #[test]
    fn an_identifier_ending_in_r_or_b_is_not_a_string_prefix() {
        // Arrange
        let source = "let bar\"x\"; let sub\"y\"; unsafe {}";

        // Act
        let found = tokens(source);

        // Assert
        assert_eq!(found, ["let", "bar", "let", "sub", "unsafe"]);
    }

    #[test]
    fn newlines_survive_so_that_line_numbers_do() {
        // Act
        let output = rust_code_only("a // x\nb /* y\nz */ c \"s\ns\" d");

        // Assert
        assert_eq!(output.lines().count(), 4);
    }

    #[test]
    fn hash_comments_are_removed_from_configuration_text() {
        // Act
        let cleaned = without_hash_comments("key = \"v#1\" # note\n# only = \"comment\"\nother = 2");

        // Assert
        assert_eq!(cleaned, "key = \"v#1\"\n\nother = 2");
    }

    #[test]
    fn a_code_span_with_backticks_in_its_info_string_does_not_open_a_fence() {
        // Act
        let states: Vec<bool> = lines_with_fence_state("```adiungere probes``` reads it.\nnext")
            .into_iter()
            .map(|(_, _, inside)| inside)
            .collect();

        // Assert
        assert_eq!(states, [false, false]);
    }

    #[test]
    fn a_longer_fence_contains_a_shorter_one() {
        // Act
        let states: Vec<bool> = lines_with_fence_state("````\n```\nx\n```\n````\nout")
            .into_iter()
            .map(|(_, _, inside)| inside)
            .collect();

        // Assert
        assert_eq!(states, [true, true, true, true, true, false]);
    }
}
