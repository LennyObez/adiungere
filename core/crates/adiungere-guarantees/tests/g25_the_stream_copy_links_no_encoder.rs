//! **G25.** The stream copy links no encoder: no encoder crate resolves in the dependency graph, and the
//! release command line carries no symbol of one.
//!
//! A lossless export is a copy of the recorder's samples. A binary that links an encoder could re-encode
//! them by a default, a convenience path or a mistake, and a file that went through an encoder is no longer
//! the evidence it was. Two lines hold this. The first is the dependency graph as `cargo metadata` resolves
//! it, which must contain none of the crates in `tools/forbidden-encoder-crates.txt`. The second is the
//! binary itself: `scripts/check-encoder-symbols.sh` reads every symbol of the release command line with
//! the toolchain's symbol reader and refuses any that carries a fragment from
//! `tools/forbidden-encoder-symbols.txt`. That script judges a fixture carrying one forbidden name before it
//! judges the product, and this guarantee runs that judgement, holds the lists non-empty, and holds the
//! script wired into the gate and the pipeline, so the check on the binary is one that runs and one that
//! has been seen to find something.

use std::process::Command;

use adiungere_guarantees::{read_at, repository_root, without_hash_comments};

/// The entries of one of the forbidden lists, without its comments and blank lines.
fn entries_of(list: &str) -> Vec<String> {
    list.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect()
}

/// Whether a crate name is one of the forbidden ones, exactly or as a binding named after it.
fn is_forbidden(name: &str, forbidden: &[String]) -> bool {
    forbidden.iter().any(|entry| {
        name == entry || name.starts_with(&format!("{entry}-")) || name.starts_with(&format!("{entry}_"))
    })
}

/// The name of every package in the resolved workspace graph, as `cargo metadata` reports it.
fn resolved_package_names() -> Result<Vec<String>, String> {
    let output = Command::new("cargo")
        .arg("metadata")
        .arg("--manifest-path")
        .arg(repository_root().join("core/Cargo.toml"))
        .args(["--locked", "--format-version", "1"])
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    let packages = metadata
        .get("packages")
        .and_then(serde_json::Value::as_array)
        .ok_or("the metadata carries no package list")?;
    packages
        .iter()
        .map(|package| {
            package
                .get("name")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
                .ok_or_else(|| "a package has no name".to_owned())
        })
        .collect()
}

#[test]
fn the_dependency_graph_resolves_no_encoder_crate() {
    // Arrange
    let forbidden = entries_of(&read_at("tools/forbidden-encoder-crates.txt").unwrap());
    let names = resolved_package_names().unwrap();

    // Act
    let offending: Vec<&String> = names
        .iter()
        .filter(|name| is_forbidden(name, &forbidden))
        .collect();

    // Assert
    assert!(
        forbidden.len() >= 15,
        "only {} crates are forbidden; this check is inert",
        forbidden.len()
    );
    assert!(
        names.len() >= 20,
        "only {} packages resolved; this check is inert",
        names.len()
    );
    assert!(
        offending.is_empty(),
        "These crates of the resolved graph link or bundle an encoder: {offending:?}"
    );
}

#[test]
fn the_crate_match_catches_a_binding_named_after_an_encoder_and_nothing_else() {
    // A detection test: the match has to see a `-sys` binding under the encoder's name, and it must not
    // see an unrelated crate whose name merely begins with the same letters.

    // Arrange
    let forbidden = entries_of(&read_at("tools/forbidden-encoder-crates.txt").unwrap());

    // Act
    let bindings = ["x264-sys", "openh264_sys2", "ffmpeg-sys-next", "gstreamer-video"]
        .iter()
        .filter(|name| is_forbidden(name, &forbidden))
        .count();
    let unrelated = ["serde", "clap", "opusfile-reader", "x2640"]
        .iter()
        .filter(|name| is_forbidden(name, &forbidden))
        .count();

    // Assert
    assert_eq!(bindings, 4, "every binding named after an encoder is matched");
    assert_eq!(unrelated, 0, "no unrelated crate is matched");
}

#[test]
fn the_toolchain_carries_the_symbol_reader() {
    // Arrange
    let toolchain = without_hash_comments(&read_at("rust-toolchain.toml").unwrap());

    // Act
    let components = toolchain
        .lines()
        .find(|line| line.trim_start().starts_with("components"))
        .map(str::to_owned);

    // Assert
    assert!(
        components
            .as_deref()
            .is_some_and(|line| line.contains("\"llvm-tools\"")),
        "rust-toolchain.toml must list the llvm-tools component, which provides the symbol reader: \
         {components:?}"
    );
}

#[test]
fn the_symbol_check_refuses_the_fixture_for_the_symbol_it_carries() {
    // Arrange
    let script = repository_root().join("scripts/check-encoder-symbols.sh");
    let fixture = read_at("scripts/encoder-symbol-fixture.rs").unwrap();
    let fragments = entries_of(&read_at("tools/forbidden-encoder-symbols.txt").unwrap());
    let carried = fixture
        .lines()
        .find_map(|line| line.trim().strip_prefix("pub fn "))
        .and_then(|rest| rest.split('(').next())
        .unwrap();

    // Act
    let output = Command::new("sh")
        .arg(&script)
        .arg("--self-test")
        .output()
        .unwrap();
    let printed = String::from_utf8_lossy(&output.stdout).into_owned();

    // Assert
    assert!(
        fragments.len() >= 15,
        "only {} fragments are forbidden; this check is inert",
        fragments.len()
    );
    assert!(
        fragments
            .iter()
            .any(|fragment| carried.contains(fragment.as_str())),
        "the fixture's function {carried} carries none of the forbidden fragments"
    );
    assert!(
        output.status.success(),
        "the self-test did not pass:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        printed.contains(carried),
        "the self-test did not name the fixture's symbol {carried}:\n{printed}"
    );
}

#[test]
fn the_symbol_check_runs_in_the_gate_and_in_the_pipeline() {
    // Arrange
    let gate = without_hash_comments(&read_at("scripts/gate.sh").unwrap());
    let workflow = without_hash_comments(&read_at(".github/workflows/core.yml").unwrap());

    // Act
    let in_the_gate = gate.contains("check-encoder-symbols.sh");
    let in_the_pipeline = workflow.contains("check-encoder-symbols.sh");

    // Assert
    assert!(in_the_gate, "scripts/gate.sh must run the symbol check");
    assert!(in_the_pipeline, "the core workflow must run the symbol check");
}

#[test]
fn the_symbol_list_names_the_common_framework_and_the_common_encoder() {
    // Arrange
    let fragments = entries_of(&read_at("tools/forbidden-encoder-symbols.txt").unwrap());

    // Act
    let has_whitespace = fragments
        .iter()
        .any(|fragment| fragment.contains(char::is_whitespace));

    // Assert
    assert!(!has_whitespace, "a fragment with whitespace matches nothing");
    assert!(fragments.iter().any(|fragment| fragment == "avcodec_"));
    assert!(fragments.iter().any(|fragment| fragment == "x264_encoder_"));
}
