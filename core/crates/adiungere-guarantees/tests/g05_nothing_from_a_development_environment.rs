//! **G05.** No tracked file carries a path, an address or a key from a development environment.
//!
//! Every tracked file is published. An absolute path from someone's home directory tells a reader nothing
//! about the software and something about its author. A network address tells an attacker where to look. Key
//! material tells them everything.
//!
//! The patterns are assembled from fragments so that this file does not contain what it forbids, which would
//! make the check either self-failing or self-excluding. Both are worse than a little indirection.
//!
//! One part of the publication rule is deliberately **not** enforced here: the names of the hosting stack a
//! service runs on. Writing them into the check would publish, in reassemblable form and in a file a reader
//! is more likely to open, exactly what keeping them out is meant to achieve. The leak that matters is an
//! address, and that is caught below; the rest is a review question in `CONTRIBUTING.md`. The other two parts
//! of the rule, a named past defect and prose that denigrates the product, are judgements a test cannot make.

use adiungere_guarantees::{TextFile, tracked_text_files};

/// Fragments that identify a developer's machine.
fn machine_fragments() -> Vec<String> {
    vec![
        format!("/{}/", "home"),
        format!("/{}/", "Users"),
        format!("C:{}{}", '\\', "Users"),
        format!("/{}/c/", "mnt"),
        format!("{}{}", "One", "Drive"),
        format!("{}{}", r"\\", "wsl"),
    ]
}

/// The header fragment every private key in the usual encoding shares.
fn key_marker() -> String {
    format!("{} {}", "PRIVATE", "KEY-----")
}

/// Address ranges reserved for documentation, plus loopback and the two wildcard forms.
fn is_an_allowed_address(address: &str) -> bool {
    const ALLOWED_PREFIXES: [&str; 4] = ["127.", "192.0.2.", "198.51.100.", "203.0.113."];
    const ALLOWED_EXACT: [&str; 2] = ["0.0.0.0", "255.255.255.255"];

    ALLOWED_EXACT.contains(&address) || ALLOWED_PREFIXES.iter().any(|prefix| address.starts_with(prefix))
}

/// Every dotted quad in a text, as whole tokens, excluding the allowed ranges.
fn addresses_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();

    for token in text.split(|character: char| !character.is_ascii_digit() && character != '.') {
        let parts: Vec<&str> = token.split('.').collect();

        if parts.len() != 4 {
            continue;
        }

        let well_formed = parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 3 && part.parse::<u16>().is_ok_and(|value| value <= 255)
        });

        if well_formed && !is_an_allowed_address(token) {
            found.push(token.to_owned());
        }
    }

    found
}

fn findings_in(file: &TextFile) -> Vec<String> {
    let mut findings = Vec::new();

    for fragment in machine_fragments() {
        if file.contents.to_lowercase().contains(&fragment.to_lowercase()) {
            findings.push(format!("{}: a path from a development environment", file.path));
            break;
        }
    }

    if file.contents.contains(&key_marker()) {
        findings.push(format!("{}: private key material", file.path));
    }

    for address in addresses_in(&file.contents) {
        findings.push(format!("{}: the address {address}", file.path));
    }

    findings
}

#[test]
fn no_tracked_file_carries_a_trace_of_a_development_environment() {
    // Arrange
    let files = tracked_text_files().unwrap();

    // Act
    let mut findings: Vec<String> = files.iter().flat_map(findings_in).collect();
    findings.sort();

    // Assert
    assert!(
        findings.is_empty(),
        "Every tracked file is published. These carry something that should not be:\n  {}",
        findings.join("\n  ")
    );
}

#[test]
fn the_patterns_detect_what_they_describe() {
    // A pattern that matches nothing makes the guarantee silently vacuous, so each is exercised against a
    // string it must catch. Every sample is assembled rather than written out.

    // Arrange
    let samples = [
        format!("/{}/someone/project/file.rs", "home"),
        format!("/{}/someone/project/file.rs", "Users"),
        format!("C:{}{}{}someone", '\\', "Users", '\\'),
        format!("/{}/c/projects", "mnt"),
        format!("{}{}/Documents", "One", "Drive"),
        format!("{}{}$/share", r"\\", "wsl"),
    ];
    let fragments = machine_fragments();

    // Act
    let unmatched: Vec<&String> = samples
        .iter()
        .filter(|sample| {
            !fragments
                .iter()
                .any(|fragment| sample.to_lowercase().contains(&fragment.to_lowercase()))
        })
        .collect();

    // Assert
    assert!(
        unmatched.is_empty(),
        "These samples should have been caught: {unmatched:?}"
    );
}

#[test]
fn the_address_check_detects_a_real_address_and_ignores_a_version() {
    // Arrange
    let address = format!("{}.{}.{}.{}", 10, 0, 0, 5);
    let version = "clap 4.6.6 and serde 1.0.229";
    let documentation = format!("{}.{}.{}.{}", 192, 0, 2, 1);

    // Act
    let caught = addresses_in(&address);
    let versions = addresses_in(version);
    let reserved = addresses_in(&documentation);

    // Assert
    assert_eq!(caught, vec![address], "A routable address must be caught.");
    assert!(
        versions.is_empty(),
        "A version number must not be read as an address: {versions:?}"
    );
    assert!(
        reserved.is_empty(),
        "A documentation address is allowed: {reserved:?}"
    );
}

#[test]
fn the_key_check_detects_a_header() {
    // Arrange
    let header = format!("-----BEGIN OPENSSH {} {}", "PRIVATE", "KEY-----");

    // Act
    let caught = header.contains(&key_marker());

    // Assert
    assert!(caught, "A private key header must be caught.");
}

#[test]
fn an_ordinary_repository_relative_path_is_not_flagged() {
    // The complement of the tests above: a pattern matching everything would be just as useless.

    // Arrange
    let innocent = TextFile {
        path: "sample".to_owned(),
        contents: "core/crates/adiungere-cli/src/evidence.rs and apps/site/public/index.html".to_owned(),
    };

    // Act
    let findings = findings_in(&innocent);

    // Assert
    assert!(
        findings.is_empty(),
        "A repository-relative path must not be flagged: {findings:?}"
    );
}

#[test]
fn the_scan_reads_real_tracked_files() {
    // Act
    let files = tracked_text_files().unwrap();

    // Assert
    assert!(
        !files.is_empty(),
        "No tracked file was read; the check above is inert."
    );
}
