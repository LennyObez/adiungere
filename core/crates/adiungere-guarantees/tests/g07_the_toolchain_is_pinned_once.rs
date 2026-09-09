//! **G07.** One toolchain pin, patch-exact, and no pipeline that restates it.
//!
//! Two pins drift apart, and the drift is silent: the pipeline stays green on a compiler nobody develops
//! against. A pipeline that names a version in its own configuration is a second pin wearing a different hat.
//!
//! Patch-exact matters for the same reason every dependency here is exact. A build of a given commit must
//! resolve the same compiler every time, because this product's central claim is that the same input produces
//! the same bytes.

use adiungere_guarantees::{read_at, tracked_paths, workflow_files};

/// Reads the value of a top-level key from a simple configuration file.
fn value_of(source: &str, key: &str) -> Option<String> {
    source.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;

        (name.trim() == key).then(|| value.trim().trim_matches('"').to_owned())
    })
}

fn is_patch_exact(channel: &str) -> bool {
    let parts: Vec<&str> = channel.split('.').collect();

    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

fn version_parts(version: &str) -> Vec<u32> {
    version.split('.').filter_map(|part| part.parse().ok()).collect()
}

#[test]
fn exactly_one_toolchain_pin_exists() {
    // Arrange
    let paths = tracked_paths().unwrap();

    // Act
    let pins: Vec<&String> = paths
        .iter()
        .filter(|path| {
            let name = path.rsplit('/').next().unwrap_or(path);
            name == "rust-toolchain.toml" || name == "rust-toolchain"
        })
        .collect();

    // Assert
    assert_eq!(
        pins.len(),
        1,
        "A compiler version must be written in exactly one place. Found: {pins:?}"
    );
    assert_eq!(
        pins.first().map(|path| path.as_str()),
        Some("rust-toolchain.toml")
    );
}

#[test]
fn the_pin_is_patch_exact() {
    // Arrange
    let source = read_at("rust-toolchain.toml").unwrap();
    let channel = value_of(&source, "channel").unwrap();

    // Act
    let exact = is_patch_exact(&channel);

    // Assert
    assert!(
        exact,
        "The channel is \"{channel}\". A moving channel means two builds of one commit can use two \
         compilers."
    );
}

#[test]
fn no_workflow_restates_the_version() {
    // Arrange
    let source = read_at("rust-toolchain.toml").unwrap();
    let channel = value_of(&source, "channel").unwrap();
    let workflows = workflow_files().unwrap();

    // Act
    let restating: Vec<String> = workflows
        .iter()
        .filter(|file| file.contents.contains(&channel) || file.contents.contains("toolchain:"))
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        restating.is_empty(),
        "These workflows name a compiler version or pass one as an input. Let the toolchain file be the \
         only pin:\n  {}",
        restating.join("\n  ")
    );
}

#[test]
fn the_workspace_minimum_is_satisfied_by_the_pin() {
    // A minimum higher than the pin means the workspace cannot build on the compiler it pins, and the error
    // arrives at the first contributor rather than here.

    // Arrange
    let pin_source = read_at("rust-toolchain.toml").unwrap();
    let manifest = read_at("core/Cargo.toml").unwrap();
    let channel = value_of(&pin_source, "channel").unwrap();
    let declared = value_of(&manifest, "rust-version").unwrap();

    // Act
    let pin = version_parts(&channel);
    let minimum = version_parts(&declared);

    // Assert
    assert!(
        !pin.is_empty() && !minimum.is_empty(),
        "Both versions must parse: {channel}, {declared}"
    );
    assert!(
        minimum <= pin,
        "The workspace requires {declared} and the pin is {channel}, so the pin cannot build it."
    );
}

#[test]
fn the_check_detects_a_moving_channel() {
    // Act and assert
    assert!(!is_patch_exact("stable"), "A named channel is not a pin.");
    assert!(!is_patch_exact("1.98"), "A two-part version still moves.");
    assert!(
        !is_patch_exact("nightly-2026-09-01"),
        "A dated nightly is not a release pin."
    );
    assert!(is_patch_exact("1.98.1"), "A three-part version is a pin.");
}

#[test]
fn the_reader_finds_a_key_and_ignores_a_comment() {
    // Arrange
    let source = "# channel = \"stable\"\n[toolchain]\nchannel = \"1.98.1\"\n";

    // Act
    let found = value_of(source, "channel");

    // Assert
    assert_eq!(
        found.as_deref(),
        Some("1.98.1"),
        "The commented line must not win."
    );
}

#[test]
fn the_scan_reads_real_workflows() {
    // Act
    let workflows = workflow_files().unwrap();

    // Assert
    assert!(
        !workflows.is_empty(),
        "No workflow was read; the check above is inert."
    );
}
