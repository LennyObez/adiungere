//! **G08.** One licence, declared the same way everywhere, with the canonical text and a dependency policy
//! that admits exactly the licences this project accepts.
//!
//! Three failures are being prevented, and they are not the same failure.
//!
//! A licence named inconsistently across the licence file, the workspace manifest, every crate manifest
//! and the citation metadata gives an organisation's counsel several answers to one question, which in
//! practice means the answer is no.
//!
//! A licence text edited by accident stops being the licence it claims to be. Nobody edits one on purpose;
//! people reflow it, or a tool trims its trailing whitespace, or a well-meaning hand changes one word. The
//! text is therefore compared by digest against the canonical publication, not by a handful of markers.
//!
//! And a dependency under terms this project cannot carry would make the outbound licence undeliverable.
//! The policy is an allow list, so this checks the allow list against the set of identifiers the project
//! accepts, in every place that list is written. Whether the resolved graph obeys the policy is the
//! supply-chain step's job, against the real dependency closure.

use adiungere_guarantees::{TextFile, read_at, tracked_text_files, without_hash_comments};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const IDENTIFIER: &str = "Apache-2.0";

/// The digest of the licence text as its publisher distributes it, which begins with a blank line the
/// repository's copy omits.
const CANONICAL_DIGEST: &str = "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30";

/// The licences this project accepts in its dependency graph. Permissive, or copyleft limited to the file
/// it covers; nothing that reaches the work as a whole.
const ACCEPTED: [&str; 11] = [
    "0BSD",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "CC0-1.0",
    "ISC",
    "MIT",
    "MPL-2.0",
    "Unicode-3.0",
    "Zlib",
];

/// The identifiers listed between `allow = [` and `]` in a policy file, comments removed.
fn allow_list_of(policy: &str) -> BTreeSet<String> {
    let cleaned = without_hash_comments(policy);
    let Some((_, after)) = cleaned.split_once("allow = [") else {
        return BTreeSet::new();
    };
    let Some((inside, _)) = after.split_once(']') else {
        return BTreeSet::new();
    };

    inside
        .split(',')
        .map(|entry| entry.trim().trim_matches('"').to_owned())
        .filter(|entry| !entry.is_empty())
        .collect()
}

/// The identifiers a workflow passes as `allow-licenses`, comments removed.
fn workflow_allow_list(workflow: &str) -> BTreeSet<String> {
    let cleaned = without_hash_comments(workflow);
    let Some((_, after)) = cleaned.split_once("allow-licenses:") else {
        return BTreeSet::new();
    };

    after
        .lines()
        .skip_while(|line| line.trim() == ">-" || line.trim().is_empty())
        .take_while(|line| line.starts_with("            "))
        .flat_map(|line| line.split(','))
        .map(|entry| entry.trim().to_owned())
        .filter(|entry| !entry.is_empty())
        .collect()
}

fn accepted() -> BTreeSet<String> {
    ACCEPTED.iter().map(|&s| s.to_owned()).collect()
}

fn hex_of(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    bytes.iter().fold(String::with_capacity(64), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

#[test]
fn the_licence_text_is_the_canonical_one_to_the_byte() {
    // Arrange
    let text = read_at("LICENSE").unwrap();

    // Act
    let digest = hex_of(&Sha256::digest(format!("\n{text}").as_bytes()));

    // Assert
    assert_eq!(
        digest, CANONICAL_DIGEST,
        "The licence text differs from the canonical publication. Replace it with the unaltered original \
         rather than repairing it."
    );
}

#[test]
fn the_attribution_notice_exists_and_says_something() {
    // The licence requires a redistributor to carry this file. An empty one satisfies nothing.

    // Arrange
    let notice = read_at("NOTICE").unwrap();

    // Assert
    assert!(notice.trim().len() > 40, "The notice file is empty or nearly so.");
    assert!(
        notice.contains("adiungere"),
        "The notice does not name the product."
    );
}

#[test]
fn every_declaration_names_the_same_licence() {
    // Arrange
    let manifest = without_hash_comments(&read_at("core/Cargo.toml").unwrap());
    let citation = without_hash_comments(&read_at("CITATION.cff").unwrap());
    let readme = read_at("README.md").unwrap();

    // Act
    let manifest_declares = manifest.contains(&format!("license = \"{IDENTIFIER}\""));
    let citation_declares = citation.contains(&format!("license: {IDENTIFIER}"));
    let readme_declares = readme.contains(IDENTIFIER);

    // Assert
    assert!(
        manifest_declares,
        "The workspace manifest does not declare {IDENTIFIER}."
    );
    assert!(
        citation_declares,
        "The citation metadata does not declare {IDENTIFIER}."
    );
    assert!(readme_declares, "The README does not name {IDENTIFIER}.");
}

#[test]
fn every_crate_inherits_the_workspace_licence() {
    // A crate may declare its own licence, and then what is published under its name differs from what the
    // repository says. Every crate takes the workspace's.

    // Arrange
    let manifests: Vec<TextFile> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.path.starts_with("core/crates/") && file.name() == "Cargo.toml")
        .collect();

    // Act
    let detached: Vec<String> = manifests
        .iter()
        .filter(|file| !without_hash_comments(&file.contents).contains("license.workspace = true"))
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        !manifests.is_empty(),
        "No crate manifest was read; this check is inert."
    );
    assert!(
        detached.is_empty(),
        "These crates declare their own licence instead of the workspace's:\n  {}",
        detached.join("\n  ")
    );
}

#[test]
fn the_dependency_policy_admits_exactly_the_accepted_licences() {
    // Arrange
    let policy = read_at("core/deny.toml").unwrap();

    // Act
    let allowed = allow_list_of(&policy);

    // Assert
    assert_eq!(
        allowed,
        accepted(),
        "The dependency policy's allow list must be exactly the set this project accepts."
    );
}

#[test]
fn the_dependency_policy_keeps_no_exception_and_no_clarification() {
    // An exception is a crate whose licence the policy refuses, kept anyway. A clarification rewrites what a
    // crate says its licence is. Either is the one place the policy can be true and meaningless at once.

    // Arrange
    let policy = without_hash_comments(&read_at("core/deny.toml").unwrap());

    // Act
    let has_empty_exceptions = policy.contains("exceptions = []");
    let has_exception_table = policy.contains("[[licenses.exceptions]]");
    let has_clarification = policy.contains("[[licenses.clarify]]");

    // Assert
    assert!(
        has_empty_exceptions,
        "The policy must state `exceptions = []` outside a comment."
    );
    assert!(
        !has_exception_table,
        "The policy carries a licence exception. Remove the dependency instead."
    );
    assert!(
        !has_clarification,
        "The policy rewrites a crate's licence. Remove the dependency instead."
    );
}

#[test]
fn the_review_pipeline_admits_the_same_licences_as_the_policy() {
    // Two lists of the same thing drift apart. The pull request review and the resolved-graph check must
    // accept exactly the same identifiers.

    // Arrange
    let workflow = read_at(".github/workflows/analysis.yml").unwrap();

    // Act
    let listed = workflow_allow_list(&workflow);

    // Assert
    assert_eq!(
        listed,
        accepted(),
        "The dependency review's allow list differs from the policy's."
    );
}

#[test]
fn the_check_detects_an_altered_text() {
    // Arrange
    let altered = read_at("LICENSE")
        .unwrap()
        .replacen("irrevocable", "revocable", 1);

    // Act
    let digest = hex_of(&Sha256::digest(format!("\n{altered}").as_bytes()));

    // Assert
    assert_ne!(
        digest, CANONICAL_DIGEST,
        "A one-word change must change the digest."
    );
}

#[test]
fn the_list_readers_read_lists_and_not_comments() {
    // Arrange
    let policy =
        "[licenses]\nallow = [\n    \"MIT\", # \"GPL-3.0\"\n    \"Apache-2.0\",\n]\n# exceptions = []\n";
    let workflow =
        "        with:\n          allow-licenses: >-\n            MIT, Apache-2.0\n          other: x\n";

    // Act
    let from_policy = allow_list_of(policy);
    let from_workflow = workflow_allow_list(workflow);

    // Assert
    assert_eq!(
        from_policy,
        ["MIT".to_owned(), "Apache-2.0".to_owned()].into_iter().collect()
    );
    assert_eq!(
        from_workflow,
        ["MIT".to_owned(), "Apache-2.0".to_owned()].into_iter().collect()
    );
    assert!(!without_hash_comments(policy).contains("exceptions = []"));
}
