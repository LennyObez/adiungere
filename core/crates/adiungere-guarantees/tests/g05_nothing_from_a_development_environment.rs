//! **G05.** No tracked file carries a path, an address, a credential or a key from a development or a
//! running environment.
//!
//! Every tracked file is published. An absolute path from someone's home directory tells a reader nothing
//! about the software and something about its author. A network address tells an attacker where to look. A
//! credential or key material tells them everything.
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

/// Fragments that identify a developer's machine, matched without regard to case.
fn machine_fragments() -> Vec<String> {
    vec![
        format!("/{}/", "home"),
        format!("/{}/", "Users"),
        format!("/{}/", "root"),
        format!(":{}{}", '\\', "Users"),
        format!(":/{}", "Users"),
        format!("/{}/c/", "mnt"),
        format!("/{}/d/", "mnt"),
        format!("/{}/e/", "mnt"),
        format!("{}{}", "One", "Drive"),
        format!("{}{}", r"\\", "wsl"),
        format!("%{}%", "USERPROFILE"),
    ]
}

/// Fragments that identify a credential or key material.
fn credential_fragments() -> Vec<String> {
    vec![
        format!("{} {}", "PRIVATE", "KEY"),
        format!("{}-{}-{}", "PuTTY", "User", "Key"),
        format!("{}_{}", "github", "pat_"),
        format!("{}{}", "gh", "p_"),
        format!("{}{}", "gh", "o_"),
        format!("{}{}", "gh", "s_"),
        format!("{}{}", "xox", "b-"),
        format!("{}{}", "AK", "IA"),
    ]
}

/// Address ranges reserved for documentation, plus loopback and the two wildcard forms.
fn is_an_allowed_v4(address: &str) -> bool {
    const ALLOWED_PREFIXES: [&str; 4] = ["127.", "192.0.2.", "198.51.100.", "203.0.113."];
    const ALLOWED_EXACT: [&str; 2] = ["0.0.0.0", "255.255.255.255"];

    ALLOWED_EXACT.contains(&address) || ALLOWED_PREFIXES.iter().any(|prefix| address.starts_with(prefix))
}

/// Every dotted quad in a text, tolerant of the punctuation prose puts after it.
fn v4_addresses_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();

    for run in text.split(|c: char| !c.is_ascii_digit() && c != '.') {
        let candidate = run.trim_matches('.');
        let parts: Vec<&str> = candidate.split('.').collect();

        if parts.len() != 4 {
            continue;
        }

        let well_formed = parts.iter().all(|part| {
            !part.is_empty() && part.len() <= 3 && part.parse::<u16>().is_ok_and(|value| value <= 255)
        });

        if well_formed && !is_an_allowed_v4(candidate) {
            found.push(candidate.to_owned());
        }
    }

    found
}

/// Every colon-separated hexadecimal address in a text: a v6 address, or a hardware address.
///
/// A time such as `12:34:56` has three groups and is not one. The documentation range and the loopback
/// address are allowed.
fn v6_addresses_in(text: &str) -> Vec<String> {
    let mut found = Vec::new();

    for run in text.split(|c: char| !c.is_ascii_hexdigit() && c != ':') {
        let candidate = run.trim_matches(':');
        let colons = run.matches(':').count();
        let groups = candidate.split(':').filter(|group| !group.is_empty()).count();
        let all_groups_short = candidate.split(':').all(|group| group.len() <= 4);
        let compressed = run.contains("::");
        // A path in source such as `de::de` is hexadecimal letters and colons too. An address carries a
        // digit; a hardware address written entirely in letters is the one thing this trades away.
        let has_a_digit = candidate.chars().any(|c| c.is_ascii_digit());

        if !all_groups_short || !has_a_digit || colons < 3 || (groups < 4 && !compressed) || groups < 2 {
            continue;
        }

        let lower = run.to_lowercase();

        if lower == "::1" || lower.starts_with("2001:db8:") || lower.starts_with("2001:0db8:") {
            continue;
        }

        found.push(run.to_owned());
    }

    found
}

fn findings_in(file: &TextFile) -> Vec<String> {
    let mut findings = Vec::new();
    let lower = file.contents.to_lowercase();

    if machine_fragments()
        .iter()
        .any(|fragment| lower.contains(&fragment.to_lowercase()))
    {
        findings.push(format!("{}: a path from a development environment", file.path));
    }

    if credential_fragments()
        .iter()
        .any(|fragment| file.contents.contains(fragment))
    {
        findings.push(format!("{}: credential or key material", file.path));
    }

    for address in v4_addresses_in(&file.contents) {
        findings.push(format!("{}: the address {address}", file.path));
    }

    for address in v6_addresses_in(&file.contents) {
        findings.push(format!("{}: the address {address}", file.path));
    }

    findings
}

#[test]
fn no_tracked_file_carries_a_trace_of_an_environment() {
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
fn the_path_patterns_detect_what_they_describe() {
    // Every sample is assembled rather than written out, so this file stays clean under its own rule.

    // Arrange
    let samples = [
        format!("/{}/someone/project/file.rs", "home"),
        format!("/{}/someone/project/file.rs", "Users"),
        format!("/{}/.cargo", "root"),
        format!("C:{}{}{}someone", '\\', "Users", '\\'),
        format!("D:/{}/someone", "Users"),
        format!("/{}/c/projects", "mnt"),
        format!("{}{}/Documents", "One", "Drive"),
        format!("{}{}$/share", r"\\", "wsl"),
        format!("%{}%/dev", "USERPROFILE"),
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
fn the_credential_patterns_detect_what_they_describe() {
    // Arrange
    let samples = [
        format!("-----BEGIN OPENSSH {} {}-----", "PRIVATE", "KEY"),
        format!("-----BEGIN PGP {} {} BLOCK-----", "PRIVATE", "KEY"),
        format!("{}-{}-{}-3: ssh-ed25519", "PuTTY", "User", "Key"),
        format!("token = \"{}_{}11AAAA\"", "github", "pat_"),
        format!("{}{}0123456789abcdef", "gh", "p_"),
    ];
    let fragments = credential_fragments();

    // Act
    let unmatched: Vec<&String> = samples
        .iter()
        .filter(|sample| !fragments.iter().any(|fragment| sample.contains(fragment)))
        .collect();

    // Assert
    assert!(
        unmatched.is_empty(),
        "These samples should have been caught: {unmatched:?}"
    );
}

#[test]
fn addresses_are_caught_with_the_punctuation_prose_puts_around_them() {
    // Arrange
    let end_of_sentence = format!("the box answers on {}.{}.{}.{}.", 10, 0, 0, 5);
    let inside_a_name = format!("see {}.{}.{}.{}.nip.io", 10, 0, 0, 5);
    let six = format!("{}:{}:{}::{}", "2a01", "e0a", "1", "1");
    let hardware = format!("{}:{}:{}:{}:{}:{}", "aa", "bb", "cc", "dd", "ee", "0f");

    // Act and assert
    assert_eq!(
        v4_addresses_in(&end_of_sentence),
        [format!("{}.{}.{}.{}", 10, 0, 0, 5)]
    );
    assert_eq!(
        v4_addresses_in(&inside_a_name),
        [format!("{}.{}.{}.{}", 10, 0, 0, 5)]
    );
    assert_eq!(v6_addresses_in(&six), [six.as_str()]);
    assert_eq!(v6_addresses_in(&hardware), [hardware.as_str()]);
}

#[test]
fn versions_times_and_reserved_ranges_are_not_addresses() {
    // The complement: a pattern matching everything would be just as useless.

    // Arrange
    let innocent = format!(
        "clap 4.6.6, serde 1.0.229, a time of 12:34:56, a ratio of 16:9, serde::de::Error, {}, {}, \
         {}:{}:{}:{}",
        "192.0.2.1", "::1", "2001", "db8", "0", "1"
    );

    // Act
    let v4 = v4_addresses_in(&innocent);
    let v6 = v6_addresses_in(&innocent);

    // Assert
    assert!(v4.is_empty(), "nothing here is an address: {v4:?}");
    assert!(v6.is_empty(), "nothing here is an address: {v6:?}");
}

#[test]
fn an_ordinary_repository_relative_path_is_not_flagged() {
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
        files.len() > 50,
        "Only {} tracked files were read; the check above is nearly inert.",
        files.len()
    );
}
