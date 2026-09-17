//! **G22.** The same inputs and the same plan write the same bytes on every platform.
//!
//! A proof of provenance that depended on which machine wrote the file would be no proof. So the digests
//! of the three export modes of the reference-like recording are pinned in the reader crate's own test
//! suite, which the core matrix runs on Linux, on macOS on both architectures and on Windows; a byte that
//! differs on one of them turns that job red. This guarantee holds two things about that mechanism: the
//! pinned digests exist and are the ones the writer produces here, and the matrix jobs run the suite that
//! carries them rather than excluding it.

use adiungere_fingerprint::Digest;
use adiungere_fixtures::{Spec, build};
use adiungere_guarantees::{read_at, without_hash_comments};
use adiungere_isobmff::{Input, RemuxPlan, SliceSource, parse, remux};

/// The pinned digests as the reader crate's test states them, read out of that file.
fn pinned_in_reader_tests() -> Result<Vec<(String, String)>, std::io::Error> {
    let source = read_at("core/crates/adiungere-isobmff/tests/remux.rs")?;
    Ok(source
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("const ")?;
            let (name, value) = rest.split_once(": &str = \"")?;
            let value = value.strip_suffix("\";")?;
            Some((name.to_owned(), value.to_owned()))
        })
        .collect())
}

fn export(bytes: &[u8], tracks: &[usize]) -> Result<Vec<u8>, adiungere_isobmff::Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: tracks.to_vec(),
    }];
    let mut out = Vec::new();
    remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true)?;
    Ok(out)
}

#[test]
fn the_reader_crate_pins_the_three_mode_digests_and_they_are_what_the_writer_produces() {
    // Arrange
    let pinned = pinned_in_reader_tests().unwrap();
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();

    // Act
    let produced = [
        (
            "FRONT_ONLY",
            Digest::of(&export(&original, &[0, 2]).unwrap()).to_string(),
        ),
        (
            "REAR_ONLY",
            Digest::of(&export(&original, &[1, 2]).unwrap()).to_string(),
        ),
        (
            "TWO_TRACK",
            Digest::of(&export(&original, &[0, 1, 2]).unwrap()).to_string(),
        ),
    ];

    // Assert
    assert_eq!(
        pinned.len(),
        3,
        "three modes are pinned in the reader crate's tests"
    );
    for (name, digest) in produced {
        let pin = pinned.iter().find(|(pinned_name, _)| pinned_name == name);
        assert_eq!(
            pin.map(|(_, value)| value.as_str()),
            Some(digest.as_str()),
            "{name}: the pinned digest is the one the writer produces here"
        );
        assert!(digest.parse::<Digest>().is_ok());
    }
}

#[test]
fn the_platform_matrix_runs_the_suite_that_carries_the_pins() {
    // Arrange
    let workflow = without_hash_comments(&read_at(".github/workflows/core.yml").unwrap());

    // Act
    let matrix_runs_tests = workflow
        .lines()
        .filter(|line| line.contains("cargo test"))
        .filter(|line| line.contains("--exclude adiungere-guarantees"))
        .count();
    let excludes_the_reader = workflow.contains("--exclude adiungere-isobmff");
    let names_the_platforms = ["macos-15", "macos-15-intel", "windows-2025"]
        .iter()
        .all(|runner| workflow.contains(runner));

    // Assert
    assert!(
        matrix_runs_tests >= 1,
        "the matrix jobs run the workspace tests less the guarantee suite"
    );
    assert!(
        !excludes_the_reader,
        "the matrix must not exclude the reader crate's tests"
    );
    assert!(
        names_the_platforms,
        "the matrix names macOS on both architectures and Windows"
    );
}

#[test]
fn the_pin_reader_reads_a_pin_and_not_prose() {
    // Act
    let pinned = pinned_in_reader_tests().unwrap();

    // Assert
    assert!(pinned.iter().all(|(name, value)| {
        name.chars().all(|c| c.is_ascii_uppercase() || c == '_') && value.len() == 64
    }));
}
