//! **G11.** No unsafe code anywhere, and no panicking construct outside a test.
//!
//! The core runs inside six host applications. A panic in it takes down the application that hosts it, on the
//! one file the person needed to look at, and a crash at that moment is indistinguishable from the product
//! saying the recording is broken.
//!
//! Unsafe code is forbidden rather than denied, so a crate cannot lift it locally. The two binding crates
//! that will need it arrive in M7 and M8, and each will opt out explicitly, at the site of use, with a
//! written reason. Until then there is none, and this says so rather than leaving it to intention.
//!
//! Tests are exempt from the panicking rule through the lint configuration, because an assertion that cannot
//! fail loudly is not an assertion. That exemption is narrow: it covers a test function, not a helper, which
//! is why every helper in this suite is pure and takes what it judges.

use adiungere_guarantees::{read_at, rust_code_only, tracked_text_files};

/// Whether Rust source, stripped of comments and literals, contains an unsafe construct.
fn contains_unsafe(source: &str) -> bool {
    rust_code_only(source)
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|token| token == "unsafe")
}

/// Lints that must be denied workspace-wide, and what each one prevents reaching a host application.
const DENIED: [(&str, &str); 6] = [
    ("unwrap_used", "a None or an Err ending the host application"),
    ("expect_used", "the same, with a message nobody reads"),
    ("panic", "a deliberate abort inside a library"),
    ("todo", "an unfinished path shipped"),
    ("unimplemented", "the same, with a different word"),
    (
        "unreachable",
        "an assumption that the input cannot reach this point",
    ),
];

#[test]
fn unsafe_code_is_forbidden_workspace_wide() {
    // Arrange
    let manifest = read_at("core/Cargo.toml").unwrap();

    // Act
    let forbidden = manifest.contains("unsafe_code = \"forbid\"");

    // Assert
    assert!(
        forbidden,
        "The workspace must forbid unsafe code, not deny it. Denying it lets a crate allow it again."
    );
}

#[test]
fn no_rust_file_contains_an_unsafe_block() {
    // The lint is the mechanism; this is the observable property. A file that reached the tree through a
    // route the lint does not cover would still be caught here.

    // Arrange
    let sources: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("rs"))
        .collect();

    // Act
    let offenders: Vec<String> = sources
        .iter()
        .filter(|file| contains_unsafe(&file.contents))
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(!sources.is_empty(), "No Rust file was read; this check is inert.");
    assert!(
        offenders.is_empty(),
        "These files contain unsafe code:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn every_panicking_construct_is_denied() {
    // Arrange
    let manifest = read_at("core/Cargo.toml").unwrap();

    // Act
    let permitted: Vec<&str> = DENIED
        .iter()
        .filter(|(lint, _)| !manifest.contains(&format!("{lint} = \"deny\"")))
        .map(|(lint, _)| *lint)
        .collect();

    // Assert
    assert!(
        permitted.is_empty(),
        "These constructs are not denied workspace-wide, so a library crate may take down the application \
         hosting it: {permitted:?}"
    );
}

#[test]
fn tests_may_still_assert_loudly() {
    // The complement of the rule above. Denying the panicking constructs without exempting tests would make
    // every assertion a lint error, and the usual answer to that is an allow attribute in every test file,
    // which is how the rule stops being enforced anywhere.

    // Arrange
    let configuration = read_at("clippy.toml").unwrap();
    let exemptions = [
        "allow-unwrap-in-tests",
        "allow-expect-in-tests",
        "allow-panic-in-tests",
    ];

    // Act
    let missing: Vec<&str> = exemptions
        .iter()
        .filter(|key| !configuration.contains(*key))
        .copied()
        .collect();

    // Assert
    assert!(missing.is_empty(), "Tests must be allowed to assert: {missing:?}");
}

#[test]
fn every_crate_inherits_the_workspace_lints() {
    // A crate that does not inherit them is a crate the rules above do not reach, and nothing else would say
    // so.

    // Arrange
    let manifests: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.path.starts_with("core/crates/") && file.path.ends_with("/Cargo.toml"))
        .collect();

    // Act
    let detached: Vec<String> = manifests
        .iter()
        .filter(|file| !file.contents.contains("[lints]\nworkspace = true"))
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        !manifests.is_empty(),
        "No crate manifest was read; this check is inert."
    );
    assert!(
        detached.is_empty(),
        "These crates do not inherit the workspace lints:\n  {}",
        detached.join("\n  ")
    );
}

#[test]
fn the_check_detects_an_unsafe_block() {
    // Arrange
    let sample = "fn read() {\n    unsafe {\n        transmute(value)\n    }\n}";

    // Act
    let caught = contains_unsafe(sample);

    // Assert
    assert!(caught, "An unsafe block must be caught.");
}

#[test]
fn the_check_reads_code_and_not_prose_about_code() {
    // Without this, the file explaining why there is no unsafe code anywhere would be the file reported for
    // containing it, and the usual fix for that is to exempt the guarantee's own file, which ends the
    // guarantee.

    // Arrange
    let comment = "// Unsafe code is forbidden here; unsafe blocks appear nowhere.\nfn read() {}";
    let literal = "fn describe() -> &'static str {\n    \"unsafe\"\n}";
    let documentation = "/// Forbids `unsafe` in every crate.\npub fn policy() {}";

    // Act and assert
    assert!(
        !contains_unsafe(comment),
        "A line comment must not be read as code."
    );
    assert!(
        !contains_unsafe(literal),
        "A string literal must not be read as code."
    );
    assert!(
        !contains_unsafe(documentation),
        "A documentation comment must not be read as code."
    );
}
