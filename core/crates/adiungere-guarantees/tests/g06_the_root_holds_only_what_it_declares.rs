//! **G06.** The repository root holds only the files it declares, and every declared file exists.
//!
//! The root is the first thing a reader sees, and it is where clutter accumulates: a configuration file for a
//! tool nobody uses any more, a note somebody meant to move, a lock file for an ecosystem that left. Keeping
//! it to a declared list is how it stays readable.
//!
//! The list is checked in both directions. Declaring a file that does not exist would be a list nobody
//! maintains, which is worse than no list: it reads as a guarantee and holds nothing.

use adiungere_guarantees::{repository_root, tracked_paths};

/// Every file the root is allowed to hold, and why it is there.
const DECLARED: [(&str, &str); 14] = [
    (".editorconfig", "editor settings every contributor inherits"),
    (
        ".gitattributes",
        "line endings, diff drivers and generated artefacts",
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

fn files_at_the_root(paths: &[String]) -> Vec<String> {
    let mut found: Vec<String> = paths
        .iter()
        .filter(|path| !path.contains('/'))
        .map(String::clone)
        .collect();

    found.sort();
    found
}

#[test]
fn the_root_holds_nothing_it_does_not_declare() {
    // Arrange
    let paths = tracked_paths().unwrap();
    let present = files_at_the_root(&paths);

    // Act
    let undeclared: Vec<&String> = present
        .iter()
        .filter(|path| !DECLARED.iter().any(|(declared, _)| *declared == path.as_str()))
        .collect();

    // Assert
    assert!(
        undeclared.is_empty(),
        "These files sit at the repository root and the root does not declare them. Move them into a \
         directory, or add them to the list in this test with the reason they belong there:\n  {}",
        undeclared
            .iter()
            .map(|path| path.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn every_declared_file_exists() {
    // A list with a stale entry reads as a guarantee and holds nothing.

    // Arrange
    let root = repository_root();

    // Act
    let missing: Vec<&str> = DECLARED
        .iter()
        .filter(|(name, _)| !root.join(name).is_file())
        .map(|(name, _)| *name)
        .collect();

    // Assert
    assert!(
        missing.is_empty(),
        "The root declares these files and they do not exist: {missing:?}"
    );
}

#[test]
fn every_declared_file_says_why_it_is_there() {
    // A list of names teaches nobody why the root looks the way it does.

    // Act
    let unexplained: Vec<&str> = DECLARED
        .iter()
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
fn the_list_names_each_file_once() {
    // A duplicated entry would let a second file of the same name pass unnoticed if one were ever removed.

    // Arrange
    let mut names: Vec<&str> = DECLARED.iter().map(|(name, _)| *name).collect();
    let total = names.len();

    // Act
    names.sort_unstable();
    names.dedup();

    // Assert
    assert_eq!(names.len(), total, "The declared list repeats an entry.");
}

#[test]
fn the_scan_reads_real_root_files() {
    // Act
    let paths = tracked_paths().unwrap();
    let present = files_at_the_root(&paths);

    // Assert
    assert!(
        !present.is_empty(),
        "No root file was read; the check above is inert."
    );
    assert!(
        present.iter().any(|path| path == "README.md"),
        "The scan did not reach the README, so it is not reading the root."
    );
}
