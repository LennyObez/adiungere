//! **G11.** No unsafe code anywhere, no panicking construct outside a test, and no attribute that switches
//! either rule off.
//!
//! The core runs inside six host applications. A panic in it takes down the application that hosts it, on the
//! one file the person needed to look at, and a crash at that moment is indistinguishable from the product
//! saying the recording is broken.
//!
//! Unsafe code is forbidden rather than denied, so a crate cannot lift it locally. The two binding crates
//! that will need it do not inherit the workspace table; each restates it with unsafe code denied rather than
//! forbidden and justifies every site, and each is named in the list below with the record that admits it.
//! Until they exist the list is empty, and this says so rather than leaving it to intention.
//!
//! Tests are exempt from the panicking rule through the lint configuration, because an assertion that cannot
//! fail loudly is not an assertion. That exemption is narrow: it covers a test function, not a helper, which
//! is why every helper in this suite is pure and takes what it judges.
//!
//! A source-level `allow` attribute would switch any of this off for one crate, silently. None is permitted
//! for the lints named here, in any file.

use adiungere_guarantees::{read_at, rust_code_only, tracked_text_files, without_hash_comments};

/// Whether Rust source, stripped of comments and literals, contains an unsafe construct.
fn contains_unsafe(source: &str) -> bool {
    rust_code_only(source)
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .any(|token| token == "unsafe")
}

/// Lints that must be denied workspace-wide, and what each one prevents reaching a host application.
const DENIED: [(&str, &str); 8] = [
    ("unwrap_used", "a None or an Err ending the host application"),
    ("expect_used", "the same, with a message nobody reads"),
    ("panic", "a deliberate abort inside a library"),
    ("todo", "an unfinished path shipped"),
    ("unimplemented", "the same, with a different word"),
    (
        "unreachable",
        "an assumption that the input cannot reach this point",
    ),
    (
        "indexing_slicing",
        "an index into a table the file described wrongly",
    ),
    (
        "integer_division",
        "a timescale conversion that lost precision without saying so",
    ),
];

/// Crates allowed to restate the lint table instead of inheriting it, with the record that admits each.
///
/// The fuzzing project is not a workspace member: it links a runtime the product does not ship and is
/// built on the dated nightly rather than the pinned compiler, so it cannot inherit the table and restates
/// it, with unsafe code forbidden as everywhere else.
const RESTATING_CRATES: [(&str, &str); 1] = [("core/fuzz/Cargo.toml", "docs/testing.md")];

/// The lint names an `allow` attribute may never name.
fn switches_a_rule_off(attribute: &str) -> bool {
    attribute.contains("unsafe_code")
        || DENIED
            .iter()
            .any(|(lint, _)| attribute.contains(&format!("clippy::{lint}")))
}

/// Every `allow(...)` attribute in Rust source that names a rule this guarantee holds.
fn forbidden_allows_in(source: &str) -> Vec<String> {
    let code = rust_code_only(source);
    let mut found = Vec::new();
    let mut rest = code.as_str();

    while let Some(start) = rest.find("allow(") {
        let after = rest.get(start + 6..).unwrap_or("");
        let end = after.find(')').unwrap_or(after.len());
        let inside = after.get(..end).unwrap_or("");

        if switches_a_rule_off(inside) {
            found.push(format!(
                "allow({})",
                inside.split_whitespace().collect::<Vec<_>>().join(" ")
            ));
        }

        rest = after.get(end..).unwrap_or("");
    }

    found
}

#[test]
fn unsafe_code_is_forbidden_workspace_wide() {
    // Arrange
    let manifest = without_hash_comments(&read_at("core/Cargo.toml").unwrap());

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
    let manifest = without_hash_comments(&read_at("core/Cargo.toml").unwrap());

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
fn no_source_switches_a_rule_off() {
    // Arrange
    let sources: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.has_extension("rs"))
        .collect();

    // Act
    let offenders: Vec<String> = sources
        .iter()
        .flat_map(|file| {
            forbidden_allows_in(&file.contents)
                .into_iter()
                .map(move |attribute| format!("{}: {attribute}", file.path))
        })
        .collect();

    // Assert
    assert!(
        offenders.is_empty(),
        "These attributes switch off a rule this guarantee holds. Fix the cause instead:\n  {}",
        offenders.join("\n  ")
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
        "allow-indexing-slicing-in-tests",
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
fn every_crate_inherits_the_workspace_lints_or_is_named_as_restating_them() {
    // A crate that does not inherit them is a crate the rules above do not reach, and nothing else would say
    // so. A crate that restates them is named with the record that admits it.

    // Arrange
    let manifests: Vec<_> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| {
            file.name() == "Cargo.toml" && without_hash_comments(&file.contents).contains("[package]")
        })
        .collect();

    // Act
    let detached: Vec<String> = manifests
        .iter()
        .filter(|file| {
            let cleaned = without_hash_comments(&file.contents);
            let inherits = cleaned.contains("[lints]\nworkspace = true");
            let restates = RESTATING_CRATES.iter().any(|(path, _)| *path == file.path)
                && (cleaned.contains("unsafe_code = \"deny\"")
                    || cleaned.contains("unsafe_code = \"forbid\""))
                && DENIED
                    .iter()
                    .all(|(lint, _)| cleaned.contains(&format!("{lint} = \"deny\"")));

            !inherits && !restates
        })
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        !manifests.is_empty(),
        "No crate manifest was read; this check is inert."
    );
    assert!(
        detached.is_empty(),
        "These crates neither inherit the workspace lints nor restate them as the record admits:\n  {}",
        detached.join("\n  ")
    );
}

#[test]
fn every_restating_crate_that_is_named_exists() {
    // A stale exemption is an exemption for a crate that could reappear under that name unreviewed.

    // Arrange
    let manifests: Vec<String> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.name() == "Cargo.toml")
        .map(|file| file.path)
        .collect();

    // Act
    let stale: Vec<&str> = RESTATING_CRATES
        .iter()
        .filter(|(path, _)| !manifests.iter().any(|manifest| manifest == path))
        .map(|(path, _)| *path)
        .collect();

    // Assert
    assert!(stale.is_empty(), "These named crates do not exist: {stale:?}");
}

#[test]
fn the_check_detects_an_unsafe_block_wherever_it_hides() {
    // Arrange
    let plain = "fn read() {\n    unsafe {\n        transmute(value)\n    }\n}";
    let after_a_lifetime = "fn f() -> &'static str { unsafe { g() } }";

    // Act and assert
    assert!(contains_unsafe(plain), "An unsafe block must be caught.");
    assert!(
        contains_unsafe(after_a_lifetime),
        "A lifetime must not hide what follows it."
    );
}

#[test]
fn the_check_reads_code_and_not_prose_about_code() {
    // Without this, the file explaining why there is no unsafe code anywhere would be the file reported for
    // containing it, and the usual fix for that is to exempt the guarantee's own file, which ends the
    // guarantee.

    // Arrange
    let comment = "// Unsafe code is forbidden here; unsafe blocks appear nowhere.\nfn read() {}";
    let literal = "fn describe() -> String { \"unsafe\".into() }";
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

#[test]
fn the_check_detects_an_attribute_that_switches_a_rule_off() {
    // Arrange
    let crate_level = "#![allow(clippy::unwrap_used, clippy::panic)]\nfn f() {}";
    let item_level = "#[allow(unsafe_code)]\nmod m {}";
    let harmless = "#[allow(dead_code)]\nfn f() {}\n// #![allow(clippy::panic)] in a comment";

    // Act and assert
    assert_eq!(forbidden_allows_in(crate_level).len(), 1);
    assert_eq!(forbidden_allows_in(item_level).len(), 1);
    assert!(
        forbidden_allows_in(harmless).is_empty(),
        "An unrelated allow, or one in a comment, is fine."
    );
}
