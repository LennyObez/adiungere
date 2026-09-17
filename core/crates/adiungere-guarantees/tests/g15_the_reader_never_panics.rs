//! **G15.** The reader never panics on any input: fuzzed nightly, smoked on every pull request.
//!
//! The core runs inside six host applications, and a panic on a hostile or merely damaged file takes the
//! application down on the file the person needed. Three things hold the line. The workspace denies every
//! panicking construct outside tests. A fuzzer runs every parser on a dated nightly, seeded with the
//! corpus, and the nightly pipeline runs every target the fuzzing project declares. And this guarantee runs
//! a deterministic mutation pass on every pull request, so that a panic cannot reach the default branch
//! between two nights.

use adiungere_fingerprint::{annex_b_digest, structural_fingerprint, track_fingerprint};
use adiungere_fixtures::corpus;
use adiungere_guarantees::{extension_of, read_at, tracked_paths};
use adiungere_isobmff::{SliceSource, parse};

/// The parsers a fuzz target must exist for, by the symbol the target has to call.
const PARSERS: [(&str, &str); 4] = [
    ("the box reader", "parse("),
    ("the sample tables", ".samples()"),
    ("the manifest reader", "Manifest::from_json"),
    ("the naming grammars", "parse_name("),
];

/// The targets the fuzzing project declares, from its manifest.
fn declared_targets(manifest: &str) -> Vec<String> {
    manifest
        .lines()
        .filter_map(|line| {
            let value = line.trim().strip_prefix("name = \"")?;
            let name = value.strip_suffix('"')?;
            (name != "adiungere-fuzz").then(|| name.to_owned())
        })
        .collect()
}

#[test]
fn a_fuzz_target_exists_for_every_parser() {
    // Arrange
    let paths = tracked_paths().unwrap();
    let targets: Vec<String> = paths
        .iter()
        .filter(|path| {
            path.starts_with("core/fuzz/fuzz_targets/") && extension_of(path).as_deref() == Some("rs")
        })
        .map(|path| read_at(path).unwrap())
        .collect();

    // Act
    let uncovered: Vec<&str> = PARSERS
        .iter()
        .filter(|(_, symbol)| !targets.iter().any(|target| target.contains(symbol)))
        .map(|(parser, _)| *parser)
        .collect();

    // Assert
    assert!(
        targets.len() >= 4,
        "only {} targets were read; this check is inert",
        targets.len()
    );
    assert!(
        uncovered.is_empty(),
        "No fuzz target exercises {uncovered:?}; a parser nobody fuzzes is a parser that panics in a \
         phone"
    );
}

#[test]
fn every_declared_target_has_its_source_and_is_run_by_the_nightly_pipeline() {
    // Arrange
    let declared = declared_targets(&read_at("core/fuzz/Cargo.toml").unwrap());
    let paths = tracked_paths().unwrap();
    let nightly = read_at(".github/workflows/nightly.yml").unwrap();

    // Act
    let without_source: Vec<&String> = declared
        .iter()
        .filter(|name| !paths.contains(&format!("core/fuzz/fuzz_targets/{name}.rs")))
        .collect();
    let not_run: Vec<&String> = declared
        .iter()
        .filter(|name| !nightly.contains(&format!("- {name}")) && !nightly.contains(&format!("\"{name}\"")))
        .collect();

    // Assert
    assert!(
        declared.len() >= 4,
        "only {} targets are declared; this check is inert",
        declared.len()
    );
    assert!(
        without_source.is_empty(),
        "declared without a source: {without_source:?}"
    );
    assert!(
        not_run.is_empty(),
        "these targets are declared and the nightly pipeline never runs them: {not_run:?}"
    );
}

#[test]
fn the_workspace_denies_every_panicking_construct_outside_tests() {
    // Arrange
    let manifest = read_at("core/Cargo.toml").unwrap();
    let lints = read_at("clippy.toml").unwrap();

    // Assert
    for lint in [
        "unwrap_used",
        "expect_used",
        "panic",
        "todo",
        "unimplemented",
        "unreachable",
        "indexing_slicing",
        "integer_division",
    ] {
        assert!(
            manifest.contains(&format!("{lint} = \"deny\"")),
            "the workspace does not deny clippy::{lint}"
        );
    }
    assert!(lints.contains("allow-unwrap-in-tests = true"));
    assert!(manifest.contains("overflow-checks = true"));
}

#[test]
fn a_mutation_pass_over_the_corpus_never_panics() {
    // The pull-request smoke. Every corpus recording, mutated a few hundred times deterministically, run
    // through every reader and every fingerprint. Not a substitute for the fuzzer; the floor under it.

    // Arrange
    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut runs = 0usize;

    for spec in corpus() {
        let Some(original) = adiungere_fixtures::build(&spec).unwrap().bytes() else {
            continue;
        };
        let length = original.len() as u64;

        for _ in 0..300 {
            let mut bytes = original.clone();
            for _ in 0..3 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let position = usize::try_from(state % length).unwrap();
                let value = u8::try_from((state >> 32) & 0xff).unwrap();
                if let Some(slot) = bytes.get_mut(position) {
                    *slot = value;
                }
            }
            if state & 0x8 == 0 {
                let cut = usize::try_from((state >> 8) % length).unwrap();
                bytes.truncate(cut.max(1));
            }

            // Act
            let mut source = SliceSource::new(&bytes);
            if let Ok(container) = parse(&mut source) {
                let _ = structural_fingerprint(&container);
                if let Ok(tracks) = container.tracks() {
                    for track in &tracks {
                        let _ = track_fingerprint(&mut source, &container, track);
                        let _ = annex_b_digest(&mut source, &container, track);
                    }
                }
            }
            runs += 1;
        }
    }

    // Assert
    assert!(runs >= 2000, "only {runs} mutations ran; this check is inert");
}
