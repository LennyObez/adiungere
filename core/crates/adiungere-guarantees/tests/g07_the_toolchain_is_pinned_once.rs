//! **G07.** One toolchain pin, patch-exact, agreed by every declaration, and no pipeline that overrides it.
//!
//! Two pins drift apart, and the drift is silent: the pipeline stays green on a compiler nobody develops
//! against. A pipeline that names a version in its own configuration is a second pin wearing a different
//! hat, and so is an environment variable that tells the toolchain manager to ignore the file.
//!
//! Patch-exact matters for the same reason every dependency here is exact. A build of a given commit must
//! resolve the same compiler every time, because this product's central claim is that the same input produces
//! the same bytes.

use adiungere_guarantees::{
    read_at, tracked_paths, tracked_text_files, tracked_text_files_under, without_hash_comments,
    workflow_files,
};

/// Reads the value of a top-level key from a simple configuration file, ignoring comments.
fn value_of(source: &str, key: &str) -> Option<String> {
    without_hash_comments(source).lines().find_map(|line| {
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

fn major_minor(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?;
    let minor = parts.next()?;

    Some(format!("{major}.{minor}"))
}

/// The ways a pipeline can pick a compiler other than the pinned one, matched without regard to case.
const OVERRIDES: [&str; 8] = [
    "rustup_toolchain",
    "rustup default",
    "rustup toolchain install",
    "rustup override",
    "cargo +",
    "toolchain:",
    "rust-version:",
    "msrv:",
];

/// Whether a token is a Rust release version: `1.` followed by two or three digits and an optional patch.
fn looks_like_a_rust_version(token: &str) -> bool {
    let Some(rest) = token.strip_prefix("1.") else {
        return false;
    };

    let mut parts = rest.split('.');
    let minor = parts.next().unwrap_or("");
    let patch = parts.next();
    let none_further = parts.next().is_none();

    (2..=3).contains(&minor.len())
        && minor.bytes().all(|b| b.is_ascii_digit())
        && patch.is_none_or(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        && none_further
}

fn overrides_in(source: &str) -> Vec<String> {
    let mut found = Vec::new();

    for (number, line) in source.lines().enumerate() {
        let lower = line.to_lowercase();

        if line.trim_start().starts_with('#') {
            continue;
        }

        for pattern in OVERRIDES {
            if lower.contains(pattern) {
                found.push(format!("line {}: {pattern}", number + 1));
            }
        }

        for token in line
            .split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == '[' || c == ']')
        {
            if looks_like_a_rust_version(token) {
                found.push(format!("line {}: the version {token}", number + 1));
            }
        }
    }

    found
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
fn no_pipeline_overrides_or_restates_the_toolchain() {
    // Arrange
    let workflows = workflow_files().unwrap();
    let cargo_config = read_at("core/.cargo/config.toml").unwrap();

    // Act
    let mut found: Vec<String> = workflows
        .iter()
        .flat_map(|file| {
            overrides_in(&file.contents)
                .into_iter()
                .map(move |finding| format!("{}: {finding}", file.path))
        })
        .collect();

    if cargo_config.to_lowercase().contains("rustup_toolchain") {
        found.push("core/.cargo/config.toml: sets the toolchain through the environment".to_owned());
    }

    // Assert
    assert!(
        found.is_empty(),
        "These pipelines pick or restate a compiler version. Let the toolchain file be the only pin:\n  {}",
        found.join("\n  ")
    );
}

#[test]
fn no_script_writes_a_compiler_version_of_its_own() {
    // The nightly script selects a dated channel for the fuzzer, which is a tool pinned in
    // tools/versions.toml and read from there. A script that wrote a version in its own text would be a
    // second pin, and a pipeline that called it would carry that pin without any workflow showing it.

    // Arrange
    let scripts: Vec<_> = tracked_text_files_under("scripts/")
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("sh"))
        .collect();

    // Act
    let found: Vec<String> = scripts
        .iter()
        .flat_map(|file| {
            file.contents
                .lines()
                .enumerate()
                .filter(|(_, line)| !line.trim_start().starts_with('#'))
                .flat_map(|(number, line)| {
                    let mut findings = Vec::new();
                    if line.contains("nightly-20") {
                        findings.push(format!("line {}: a dated nightly channel", number + 1));
                    }
                    for token in line.split(|c: char| c.is_whitespace() || c == '"' || c == '\'') {
                        if looks_like_a_rust_version(token) {
                            findings.push(format!("line {}: the version {token}", number + 1));
                        }
                    }
                    findings
                })
                .map(move |finding| format!("{}: {finding}", file.path))
                .collect::<Vec<_>>()
        })
        .collect();

    // Assert
    assert!(
        scripts.len() >= 5,
        "only {} scripts were read; this check is inert",
        scripts.len()
    );
    assert!(
        found.is_empty(),
        "These scripts write a compiler version in their own text:\n  {}",
        found.join("\n  ")
    );
}

#[test]
fn the_nightly_channel_is_pinned_by_date_in_the_versions_file() {
    // Arrange
    let versions = without_hash_comments(&read_at("tools/versions.toml").unwrap());

    // Act
    let channel = versions
        .lines()
        .find_map(|line| line.trim().strip_prefix("channel = \""))
        .and_then(|rest| rest.strip_suffix('"'))
        .map(str::to_owned);

    // Assert
    let channel = channel.expect("tools/versions.toml pins no nightly channel");
    let date = channel.strip_prefix("nightly-").unwrap_or_default();
    assert_eq!(
        date.len(),
        10,
        "the nightly is dated to the day, as nightly-YYYY-MM-DD: {channel}"
    );
    assert!(
        date.bytes()
            .enumerate()
            .all(|(index, byte)| if index == 4 || index == 7 {
                byte == b'-'
            } else {
                byte.is_ascii_digit()
            }),
        "the date is written as YYYY-MM-DD: {channel}"
    );
}

#[test]
fn every_minimum_declaration_agrees_with_the_pin() {
    // A minimum higher than the pin cannot build on the compiler it pins. A minimum lower than the pin is an
    // assertion nothing compiles, since no pipeline runs the minimum. So the two declarations and the pin
    // agree to the minor version, and moving one means moving all three.

    // Arrange
    let pin = value_of(&read_at("rust-toolchain.toml").unwrap(), "channel").unwrap();
    let workspace = value_of(&read_at("core/Cargo.toml").unwrap(), "rust-version").unwrap();
    let lints = value_of(&read_at("clippy.toml").unwrap(), "msrv").unwrap();

    // Act
    let expected = major_minor(&pin).unwrap();

    // Assert
    assert_eq!(
        workspace, expected,
        "core/Cargo.toml rust-version must be the pin's major and minor."
    );
    assert_eq!(
        lints, expected,
        "clippy.toml msrv must be the pin's major and minor."
    );
}

#[test]
fn every_crate_inherits_the_workspace_minimum() {
    // Arrange
    let manifests: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.path.starts_with("core/crates/") && file.name() == "Cargo.toml")
        .collect();

    // Act
    let detached: Vec<String> = manifests
        .iter()
        .filter(|file| !without_hash_comments(&file.contents).contains("rust-version.workspace = true"))
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        !manifests.is_empty(),
        "No crate manifest was read; this check is inert."
    );
    assert!(
        detached.is_empty(),
        "These crates declare their own minimum:\n  {}",
        detached.join("\n  ")
    );
}

#[test]
fn the_check_detects_every_kind_of_override() {
    // Arrange
    let samples = [
        "env:\n  RUSTUP_TOOLCHAIN: 1.97.0",
        "run: rustup default stable",
        "run: cargo +nightly fuzz run reader",
        "with:\n  toolchain: stable",
        "matrix:\n  rust: [1.96.0, 1.97.0]",
        "run: rustup toolchain install nightly",
    ];

    // Act
    let missed: Vec<&&str> = samples
        .iter()
        .filter(|sample| overrides_in(sample).is_empty())
        .collect();

    // Assert
    assert!(missed.is_empty(), "These overrides were not caught: {missed:?}");
}

#[test]
fn an_action_version_or_a_node_version_is_not_a_rust_version() {
    // Act and assert
    assert!(!looks_like_a_rust_version("v7.0.1"));
    assert!(!looks_like_a_rust_version("24.15.0"));
    assert!(!looks_like_a_rust_version("2.27.0"));
    assert!(!looks_like_a_rust_version("1.0"));
    assert!(looks_like_a_rust_version("1.98.1"));
    assert!(looks_like_a_rust_version("1.98"));
    assert!(!is_patch_exact("stable"));
    assert!(!is_patch_exact("1.98"));
    assert!(!is_patch_exact("nightly-2026-09-01"));
    assert!(is_patch_exact("1.98.1"));
}

#[test]
fn the_reader_finds_a_key_and_ignores_a_comment() {
    // Arrange
    let source = "# channel = \"stable\"\n[toolchain]\nchannel = \"1.98.1\" # pinned\n";

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
        workflows.len() >= 4,
        "Only {} workflows were read; the check above is nearly inert.",
        workflows.len()
    );
}
