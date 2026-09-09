//! **G08.** One licence, declared the same way everywhere, with an unaltered text and a policy that refuses
//! a reciprocal dependency.
//!
//! Three failures are being prevented, and they are not the same failure.
//!
//! A licence named inconsistently across the licence file, the package manifest and the citation metadata
//! gives an organisation's counsel three answers to one question, which in practice means the answer is no.
//!
//! A licence text edited by accident stops being the licence it claims to be. Nobody edits one on purpose;
//! people reflow it, or a tool trims its trailing whitespace.
//!
//! And a reciprocal dependency would make the outbound licence undeliverable. The policy refuses one by
//! having no such identifier in its allow list, which this checks; whether the resolved graph obeys the
//! policy is checked by the supply-chain step of the pipeline, against the real dependency closure.

use adiungere_guarantees::read_at;

const IDENTIFIER: &str = "Apache-2.0";

/// Structural markers of the canonical text, one per part that an edit would disturb.
const MARKERS: [&str; 6] = [
    "                                 Apache License",
    "                           Version 2.0, January 2004",
    "   TERMS AND CONDITIONS FOR USE, REPRODUCTION, AND DISTRIBUTION",
    "   5. Submission of Contributions.",
    "   END OF TERMS AND CONDITIONS",
    "   APPENDIX: How to apply the Apache License to your work.",
];

/// Licence families whose terms this project's outbound licence cannot carry.
const RECIPROCAL: [&str; 5] = ["GPL", "EPL", "SSPL", "OSL", "CPAL"];

#[test]
fn the_licence_text_is_the_canonical_one() {
    // Arrange
    let text = read_at("LICENSE").unwrap();

    // Act
    let missing: Vec<&str> = MARKERS
        .iter()
        .filter(|marker| !text.contains(*marker))
        .copied()
        .collect();

    // Assert
    assert!(
        missing.is_empty(),
        "The licence text is missing parts of the canonical text. Replace it with the unaltered original \
         rather than repairing it:\n  {}",
        missing.join("\n  ")
    );
    assert_eq!(
        text.lines().count(),
        201,
        "The canonical text is 201 lines. A different count means it was reflowed or truncated."
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
    let manifest = read_at("core/Cargo.toml").unwrap();
    let citation = read_at("CITATION.cff").unwrap();
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
fn the_dependency_policy_admits_no_reciprocal_licence() {
    // Arrange
    let policy = read_at("core/deny.toml").unwrap();

    // Act
    let admitted: Vec<&str> = RECIPROCAL
        .iter()
        .filter(|family| {
            policy
                .lines()
                .filter(|line| line.trim_start().starts_with('"'))
                .any(|line| line.contains(*family))
        })
        .copied()
        .collect();

    // Assert
    assert!(
        admitted.is_empty(),
        "The dependency policy allows a licence this project's outbound licence cannot carry: {admitted:?}"
    );
}

#[test]
fn the_dependency_policy_keeps_no_exception() {
    // An exception here is a crate whose licence the policy refuses, kept anyway. It is the one place the
    // policy can be true and meaningless at the same time.

    // Arrange
    let policy = read_at("core/deny.toml").unwrap();

    // Act
    let empty = policy.contains("exceptions = []");

    // Assert
    assert!(
        empty,
        "The dependency policy carries a licence exception. Remove the dependency instead."
    );
}

#[test]
fn the_check_detects_an_altered_text() {
    // A marker check passes vacuously if the markers are wrong, so one is exercised against a text that must
    // fail it.

    // Arrange
    let reflowed = "Apache License Version 2.0, January 2004. Terms and conditions follow.";

    // Act
    let missing: Vec<&str> = MARKERS
        .iter()
        .filter(|marker| !reflowed.contains(*marker))
        .copied()
        .collect();

    // Assert
    assert_eq!(
        missing.len(),
        MARKERS.len(),
        "A reflowed text must fail every structural marker."
    );
}

#[test]
fn the_policy_check_detects_a_reciprocal_identifier() {
    // Arrange
    let permissive = "allow = [\n    \"Apache-2.0\",\n    \"MIT\",\n]";
    let reciprocal = "allow = [\n    \"Apache-2.0\",\n    \"LGPL-2.1\",\n]";

    let admits = |policy: &str| {
        RECIPROCAL.iter().any(|family| {
            policy
                .lines()
                .filter(|line| line.trim_start().starts_with('"'))
                .any(|line| line.contains(*family))
        })
    };

    // Act and assert
    assert!(!admits(permissive), "A permissive allow list must pass.");
    assert!(admits(reciprocal), "A reciprocal identifier must be caught.");
}
