//! **G06.** The repository root holds only the files and directories it declares, and every declared one
//! exists.
//!
//! The root is the first thing a reader sees, and it is where clutter accumulates: a configuration file for a
//! tool nobody uses any more, a note somebody meant to move, a directory for an ecosystem that left. Keeping
//! it to a declared list is how it stays readable.
//!
//! The list is checked in both directions and covers directories as well as files, because a stray
//! directory is the more common way for a root to fill up. Declaring an entry that does not exist would be a
//! list nobody maintains, which is worse than no list: it reads as a guarantee and holds nothing.

use adiungere_guarantees::{repository_root, tracked_paths};
use std::collections::BTreeSet;

/// Every file the root is allowed to hold, and why it is there.
const FILES: [(&str, &str); 14] = [
    (".editorconfig", "editor settings every contributor inherits"),
    (
        ".gitattributes",
        "line endings, diff drivers, generated artefacts and what is binary",
    ),
    (
        ".gitignore",
        "what is never tracked, including signing material by extension",
    ),
    ("CHANGELOG.md", "what changed, in the format the project follows"),
    (
        "CITATION.cff",
        "how to cite the software and the specification it publishes",
    ),
    (
        "CODE_OF_CONDUCT.md",
        "how people are expected to treat each other here",
    ),
    (
        "CONTRIBUTING.md",
        "the workflow, the commit rules and the publication rule",
    ),
    ("LICENSE", "the licence text, unaltered"),
    (
        "NOTICE",
        "the attribution notice the licence requires a redistributor to carry",
    ),
    (
        "README.md",
        "what the product is, what it proves, and what it does not",
    ),
    (
        "SECURITY.md",
        "how to report a vulnerability and what is in scope",
    ),
    (
        "clippy.toml",
        "how individual lints behave, found from any directory in the checkout",
    ),
    (
        "rust-toolchain.toml",
        "the one place a compiler version is written",
    ),
    (
        "rustfmt.toml",
        "what formatted code looks like, found from any directory in the checkout",
    ),
];

/// Every directory the root is allowed to hold, and what tier it belongs to.
const DIRECTORIES: [(&str, &str); 9] = [
    (
        ".github",
        "what the platform reads: workflows, forms, labels, owners, update policy",
    ),
    (
        "apps",
        "everything that ships to a person, one directory per shipping target",
    ),
    (
        "core",
        "the Rust workspace, the one implementation of the container format",
    ),
    (
        "design",
        "the token source every platform theme is generated from",
    ),
    (
        "docs",
        "roadmap, architecture, testing, evidence register and decision records",
    ),
    ("infra", "how an environment is described and deployed"),
    (
        "scripts",
        "the gate runner and the checks that do not belong to a crate",
    ),
    (
        "tools",
        "the pinned versions of the external tools the pipeline runs as oracles",
    ),
    ("web", "the shared player and export interface"),
];

fn root_entries(paths: &[String]) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut files = BTreeSet::new();
    let mut directories = BTreeSet::new();

    for path in paths {
        match path.split_once('/') {
            Some((directory, _)) => {
                directories.insert(directory.to_owned());
            },
            None => {
                files.insert(path.clone());
            },
        }
    }

    (files, directories)
}

#[test]
fn the_root_holds_nothing_it_does_not_declare() {
    // Arrange
    let paths = tracked_paths().unwrap();
    let (files, directories) = root_entries(&paths);

    // Act
    let undeclared_files: Vec<&String> = files
        .iter()
        .filter(|file| !FILES.iter().any(|(declared, _)| *declared == file.as_str()))
        .collect();
    let undeclared_directories: Vec<&String> = directories
        .iter()
        .filter(|directory| {
            !DIRECTORIES
                .iter()
                .any(|(declared, _)| *declared == directory.as_str())
        })
        .collect();

    // Assert
    assert!(
        undeclared_files.is_empty() && undeclared_directories.is_empty(),
        "These entries sit at the repository root and the root does not declare them. Move them, or add \
         them to the list in this test with the reason they belong there:\n  files: {undeclared_files:?}\n  \
         directories: {undeclared_directories:?}"
    );
}

#[test]
fn every_declared_entry_exists() {
    // A list with a stale entry reads as a guarantee and holds nothing.

    // Arrange
    let root = repository_root();
    let paths = tracked_paths().unwrap();
    let (_, directories) = root_entries(&paths);

    // Act
    let missing_files: Vec<&str> = FILES
        .iter()
        .filter(|(name, _)| !root.join(name).is_file() || !paths.iter().any(|path| path == name))
        .map(|(name, _)| *name)
        .collect();
    let missing_directories: Vec<&str> = DIRECTORIES
        .iter()
        .filter(|(name, _)| !directories.contains(*name))
        .map(|(name, _)| *name)
        .collect();

    // Assert
    assert!(
        missing_files.is_empty() && missing_directories.is_empty(),
        "The root declares entries that are not tracked: files {missing_files:?}, directories \
         {missing_directories:?}"
    );
}

#[test]
fn every_declared_entry_says_why_it_is_there() {
    // A list of names teaches nobody why the root looks the way it does.

    // Act
    let unexplained: Vec<&str> = FILES
        .iter()
        .chain(DIRECTORIES.iter())
        .filter(|(_, reason)| reason.len() < 20)
        .map(|(name, _)| *name)
        .collect();

    // Assert
    assert!(
        unexplained.is_empty(),
        "These entries carry no real reason: {unexplained:?}"
    );
}

#[test]
fn the_lists_name_each_entry_once() {
    // A duplicated entry would let a second file of the same name pass unnoticed if one were ever removed.

    // Arrange
    let mut names: Vec<&str> = FILES
        .iter()
        .chain(DIRECTORIES.iter())
        .map(|(name, _)| *name)
        .collect();
    let total = names.len();

    // Act
    names.sort_unstable();
    names.dedup();

    // Assert
    assert_eq!(names.len(), total, "A declared list repeats an entry.");
}

#[test]
fn the_check_detects_an_undeclared_directory() {
    // Arrange
    let paths = vec!["notes/todo.md".to_owned(), "README.md".to_owned()];

    // Act
    let (_, directories) = root_entries(&paths);

    // Assert
    assert!(
        directories.contains("notes"),
        "A stray directory must be seen: {directories:?}"
    );
}

#[test]
fn the_scan_reads_real_root_entries() {
    // Act
    let paths = tracked_paths().unwrap();
    let (files, directories) = root_entries(&paths);

    // Assert
    assert!(files.contains("README.md"), "The scan did not reach the README.");
    assert!(
        directories.contains("core"),
        "The scan did not reach the workspace."
    );
}
