//! The recording commands, exercised as a person runs them, on the synthetic corpus.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use adiungere_fixtures::{Spec, build, corpus};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-cli-{name}-{}-{:?}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default()
        );
        let path = std::env::temp_dir().join(unique);
        let _ = std::fs::create_dir_all(&path);
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn run(arguments: &[&str]) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_adiungere"))
        .args(arguments)
        .output()
}

fn out(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn err(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_reference_like(directory: &Path, name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let path = directory.join(name);
    build(&Spec::reference_like())?.write_to(&path)?;
    Ok(path)
}

#[test]
fn inspect_describes_the_structure_and_never_prints_a_fingerprint() {
    // Arrange
    let directory = Temporary::new("inspect");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();

    // Act
    let output = run(&["inspect", clip.to_str().unwrap()]).unwrap();
    let printed = out(&output);

    // Assert
    assert!(output.status.success(), "{}", err(&output));
    assert!(printed.contains("3 track(s)"), "{printed}");
    assert!(
        printed.contains("2 video tracks are marked as the default"),
        "{printed}"
    );
    assert!(
        printed.contains("zvnd (30720 bytes under user data)"),
        "{printed}"
    );
    assert!(printed.contains("no later than 2026-06-04T12:23:23"), "{printed}");
    assert!(!printed.contains("fingerprint ("), "{printed}");
    assert!(printed.contains("facts about bytes"), "{printed}");
}

#[test]
fn inspect_as_json_is_a_manifest_without_digests() {
    // Arrange
    let directory = Temporary::new("inspect-json");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();

    // Act
    let output = run(&["inspect", clip.to_str().unwrap(), "--format", "json"]).unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();

    // Assert
    assert!(output.status.success(), "{}", err(&output));
    assert_eq!(parsed["format"], "adiungere-manifest/1");
    assert_eq!(parsed["file"]["sha256"], serde_json::Value::Null);
    assert_eq!(parsed["produced"]["operation"]["scope"], "structure");
    assert_eq!(parsed["tracks"].as_array().unwrap().len(), 3);
}

#[test]
fn fingerprint_writes_a_manifest_that_verify_accepts_and_a_changed_file_fails() {
    // Arrange
    let directory = Temporary::new("fingerprint");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let manifest = directory.path().join("clip.manifest.json");

    // Act
    let fingerprinted = run(&[
        "fingerprint",
        clip.to_str().unwrap(),
        "--manifest",
        manifest.to_str().unwrap(),
    ])
    .unwrap();
    let verified = run(&["verify", manifest.to_str().unwrap(), clip.to_str().unwrap()]).unwrap();

    let mut bytes = std::fs::read(&clip).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0xff;
    let changed = directory.path().join("changed.MP4");
    std::fs::write(&changed, bytes).unwrap();
    let refused = run(&["verify", manifest.to_str().unwrap(), changed.to_str().unwrap()]).unwrap();

    // Assert
    assert!(fingerprinted.status.success(), "{}", err(&fingerprinted));
    assert!(out(&fingerprinted).contains("elementary stream (adiungere-annexb/1)"));
    assert!(manifest.is_file());
    assert!(verified.status.success(), "{}", out(&verified));
    assert!(
        out(&verified).contains("identical, byte for byte"),
        "{}",
        out(&verified)
    );
    assert_eq!(refused.status.code(), Some(1), "{}", out(&refused));
    assert!(out(&refused).contains("The file differs"), "{}", out(&refused));
    assert!(
        out(&refused).contains("Track 2: the stored samples differ"),
        "the last byte belongs to the audio track's last sample: {}",
        out(&refused)
    );
    assert!(out(&refused).contains("Track 0: the stored samples are identical"));
}

#[test]
fn report_lists_the_commands_that_reproduce_each_number() {
    // Arrange
    let directory = Temporary::new("report");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let manifest = directory.path().join("clip.manifest.json");
    run(&[
        "fingerprint",
        clip.to_str().unwrap(),
        "--manifest",
        manifest.to_str().unwrap(),
        "--format",
        "json",
    ])
    .unwrap();

    // Act
    let output = run(&["report", manifest.to_str().unwrap()]).unwrap();
    let printed = out(&output);

    // Assert
    assert!(output.status.success(), "{}", err(&output));
    assert!(printed.contains("sha256sum 20260604_122323E.MP4"), "{printed}");
    assert!(
        printed.contains("scripts/track-fingerprint.py 20260604_122323E.MP4 0"),
        "{printed}"
    );
    assert!(
        printed.contains("-map 0:v:1 -c copy -f h264 - | sha256sum"),
        "{printed}"
    );
}

#[test]
fn detect_groups_a_directory_and_names_the_signs_of_a_rewrite() {
    // Arrange
    let directory = Temporary::new("detect");
    write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let rewritten = corpus()
        .into_iter()
        .find(|spec| spec.name == "rewritten")
        .unwrap();
    build(&rewritten)
        .unwrap()
        .write_to(&directory.path().join("20260604_122423E.MP4"))
        .unwrap();
    build(&rewritten)
        .unwrap()
        .write_to(&directory.path().join("20260101_101010_F.mp4"))
        .unwrap();
    build(&rewritten)
        .unwrap()
        .write_to(&directory.path().join("20260101_101010_R.mp4"))
        .unwrap();
    // The cache lives outside the scanned directory, or the scan would report the cache file itself.
    let elsewhere = Temporary::new("detect-cache");
    let cache = elsewhere.path().join("cache.json");

    // Act
    let output = run(&[
        "detect",
        directory.path().to_str().unwrap(),
        "--cache",
        cache.to_str().unwrap(),
    ])
    .unwrap();
    let printed = out(&output);
    let again = run(&[
        "detect",
        directory.path().to_str().unwrap(),
        "--cache",
        cache.to_str().unwrap(),
        "--format",
        "json",
    ])
    .unwrap();
    let parsed: serde_json::Value = serde_json::from_slice(&again.stdout).unwrap();

    // Assert
    assert!(output.status.success(), "{}", err(&output));
    assert!(printed.contains("one file holding two cameras"), "{printed}");
    assert!(printed.contains("No sign of rewriting was found"), "{printed}");
    assert!(
        printed.contains("3 sign(s) that another program wrote this file"),
        "{printed}"
    );
    assert!(
        printed.contains("two files, one per camera, matched by name"),
        "{printed}"
    );
    assert!(cache.is_file());
    assert!(again.status.success());
    assert_eq!(
        parsed["recordings"].as_array().unwrap().len(),
        3,
        "two single files and one pair: {parsed}"
    );
    assert_eq!(parsed["skipped_by_extension"], 0);
}

#[test]
fn a_file_that_is_not_a_recording_is_a_usage_failure_for_inspect_and_a_no_for_detect() {
    // Arrange
    let directory = Temporary::new("not-a-recording");
    let text = directory.path().join("20260604_122323E.MP4");
    std::fs::write(&text, b"this is not a container").unwrap();

    // Act
    let inspected = run(&["inspect", text.to_str().unwrap()]).unwrap();
    let detected = run(&["detect", text.to_str().unwrap()]).unwrap();

    // Assert
    assert_eq!(inspected.status.code(), Some(2), "{}", out(&inspected));
    assert!(
        err(&inspected).contains("does not begin with a box header"),
        "{}",
        err(&inspected)
    );
    assert_eq!(detected.status.code(), Some(1), "{}", out(&detected));
    assert!(out(&detected).contains("could not be read"), "{}", out(&detected));
}

#[test]
fn a_manifest_of_another_format_is_refused_by_verify() {
    // Arrange
    let directory = Temporary::new("foreign-manifest");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let manifest = directory.path().join("foreign.json");
    std::fs::write(&manifest, r#"{"format":"adiungere-manifest/7"}"#).unwrap();

    // Act
    let output = run(&["verify", manifest.to_str().unwrap(), clip.to_str().unwrap()]).unwrap();

    // Assert
    assert_eq!(output.status.code(), Some(2));
    assert!(
        err(&output).contains("not a manifest this version reads"),
        "{}",
        err(&output)
    );
}
