//! The command line, exercised as a person runs it.
//!
//! The parsing of the register is tested on its own elsewhere. This drives the built binary instead, because a
//! library that parses perfectly behind a command that exits with the wrong status, prints to the wrong
//! stream, or cannot be reached from another directory is a library nobody can use.
//!
//! Exit statuses are part of the contract: 0 when the answer is yes, 1 when the answer is no, 2 when the
//! question could not be asked. A pipeline that cannot tell the second from the third is a pipeline that
//! reports a green on a broken invocation.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root, three directories above this crate. No fallible step, so no panic outside a test.
fn repository_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    for _ in 0..3 {
        path.pop();
    }

    path
}

/// Runs the built binary in a directory, returning whatever the operating system reported.
fn run_in(directory: &Path, arguments: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_adiungere"))
        .args(arguments)
        .current_dir(directory)
        .output()
}

/// Runs the built binary from the repository root, which is how a person runs it.
fn run(arguments: &[&str]) -> std::io::Result<Output> {
    run_in(&repository_root(), arguments)
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn checking_the_register_against_the_roadmap_succeeds_on_this_repository() {
    // Act
    let output = run(&["probes", "check"]).unwrap();

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );
    assert!(out(&output).contains("agree"), "got {:?}", out(&output));
}

#[test]
fn listing_every_probe_prints_one_line_each_and_a_tally() {
    // Act
    let output = run(&["probes", "list"]).unwrap();
    let printed = out(&output);

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );

    let entries = printed.lines().filter(|line| line.starts_with('P')).count();
    assert!(entries >= 30, "only {entries} probes were listed:\n{printed}");
    assert!(printed.contains("probes:"), "the tally is missing:\n{printed}");
}

#[test]
fn a_filter_narrows_the_listing_and_the_tally_still_covers_the_register() {
    // The tally is deliberately about the whole register rather than the filtered view. A reader who filters
    // to one milestone should still see how much of the project is unanswered.

    // Act
    let output = run(&["probes", "list", "--milestone", "M1"]).unwrap();
    let printed = out(&output);

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );
    assert!(
        printed.contains(" of "),
        "the tally should say how many of the total were shown:\n{printed}"
    );
    assert!(
        printed
            .lines()
            .filter(|line| line.starts_with('P'))
            .all(|line| line.contains("M1"))
    );
}

#[test]
fn showing_one_probe_prints_its_question_and_says_what_is_not_recorded() {
    // Act
    let output = run(&["probes", "show", "P11"]).unwrap();
    let printed = out(&output);

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );
    assert!(printed.contains("P11"), "got {printed}");
    assert!(printed.contains("Question"), "got {printed}");
    assert!(
        printed.contains("not evidence of anything"),
        "an unanswered probe must say so rather than printing an empty result:\n{printed}"
    );
}

#[test]
fn the_machine_readable_form_is_valid_json() {
    // Act
    let output = run(&["probes", "show", "P01", "--format", "json"]).unwrap();
    let printed = out(&output);

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );

    let parsed: serde_json::Value = serde_json::from_str(&printed).expect("the output must parse as JSON");
    assert_eq!(parsed.get("id").and_then(serde_json::Value::as_str), Some("P01"));
    assert_eq!(parsed.get("result"), Some(&serde_json::Value::Null));
}

#[test]
fn an_unknown_probe_is_refused_with_a_usage_status() {
    // Act
    let output = run(&["probes", "show", "P99"]).unwrap();

    // Assert
    assert_eq!(output.status.code(), Some(2), "stderr: {}", err(&output));
    assert!(
        err(&output).contains("P99"),
        "the message must name what was asked for"
    );
}

#[test]
fn a_word_that_is_not_a_verdict_is_refused_before_anything_is_printed() {
    // Act
    let output = run(&["probes", "list", "--verdict", "probably fine"]).unwrap();

    // Assert
    assert_eq!(output.status.code(), Some(2), "stderr: {}", err(&output));
    assert!(
        out(&output).is_empty(),
        "nothing should be printed on a refused invocation"
    );
    assert!(err(&output).contains("not a verdict"), "got {}", err(&output));
}

#[test]
fn the_command_works_from_a_subdirectory() {
    // The register is found by walking upwards. Without that, the command would only work from one place and
    // every pipeline step would have to know which.

    // Act
    let output = run_in(&repository_root().join("core/crates"), &["probes", "check"]).unwrap();

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );
}

#[test]
fn outside_a_checkout_the_command_says_so_rather_than_reporting_an_empty_register() {
    // Act
    let output = run_in(&std::env::temp_dir(), &["probes", "list"]).unwrap();

    // Assert
    assert_eq!(output.status.code(), Some(2), "stdout: {}", out(&output));
    assert!(
        err(&output).contains("no repository found"),
        "got {}",
        err(&output)
    );
}

#[test]
fn a_register_given_explicitly_is_read_instead_of_the_repository() {
    // Arrange
    let root = repository_root();
    let register = root.join("docs/evidence.md");
    let roadmap = root.join("docs/roadmap.md");

    // Act
    let output = Command::new(env!("CARGO_BIN_EXE_adiungere"))
        .args(["probes", "check"])
        .arg("--register")
        .arg(&register)
        .arg("--roadmap")
        .arg(&roadmap)
        .current_dir(std::env::temp_dir())
        .output()
        .unwrap();

    // Assert
    assert!(
        output.status.success(),
        "exit {:?}: {}",
        output.status.code(),
        err(&output)
    );
}
