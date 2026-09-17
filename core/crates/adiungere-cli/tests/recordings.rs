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
        printed.contains("4 sign(s) that another program wrote this file"),
        "{printed}"
    );
    assert!(
        printed.contains("the encoder tag Lavf belongs to another program"),
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

#[test]
fn export_writes_each_camera_with_its_manifest_and_verify_accepts_the_output() {
    // Arrange
    let directory = Temporary::new("export");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let rear = directory.path().join("rear.mp4");
    let front = directory.path().join("front.mp4");
    let both = directory.path().join("both.mp4");

    // Act
    let rear_run = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        rear.to_str().unwrap(),
    ])
    .unwrap();
    let front_run = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "front",
        "--out",
        front.to_str().unwrap(),
        "--format",
        "json",
    ])
    .unwrap();
    let both_run = run(&["export", clip.to_str().unwrap(), "--out", both.to_str().unwrap()]).unwrap();

    // Assert
    assert_eq!(rear_run.status.code(), Some(0), "{}", err(&rear_run));
    assert_eq!(front_run.status.code(), Some(0), "{}", err(&front_run));
    assert_eq!(both_run.status.code(), Some(0), "{}", err(&both_run));
    let printed = out(&rear_run);
    assert!(printed.contains("rear.mp4: extraction"), "{printed}");
    assert!(
        printed.contains("Track 0 is track 1 of 20260604_122323E.MP4"),
        "{printed}"
    );
    assert!(printed.contains("carried across as bytes"), "{printed}");
    assert!(printed.contains("faces, number plates"), "{printed}");
    assert!(
        out(&both_run).contains("both.mp4: two_track_archive"),
        "{}",
        out(&both_run)
    );
    let json: serde_json::Value = serde_json::from_str(&out(&front_run)).unwrap();
    assert_eq!(json["manifest"]["produced"]["operation"]["kind"], "export");
    assert_eq!(json["manifest"]["produced"]["operation"]["class"], "extraction");
    assert_eq!(json["manifest"]["produced"]["operation"]["masking"], "none");
    assert_eq!(
        json["manifest"]["produced"]["operation"]["sources"][0]["tracks"][0]["index"],
        0
    );
    assert_eq!(json["report"]["tracks"].as_array().unwrap().len(), 2);
    for output in [&rear, &front, &both] {
        let manifest = directory.path().join(format!(
            "{}.manifest.json",
            output.file_name().unwrap().to_str().unwrap()
        ));
        assert!(manifest.is_file(), "{}", manifest.display());
        let verified = run(&["verify", manifest.to_str().unwrap(), output.to_str().unwrap()]).unwrap();
        assert_eq!(verified.status.code(), Some(0), "{}", out(&verified));
        assert!(!directory.path().join("rear.part").exists());
    }
}

#[test]
fn export_refuses_a_rear_camera_the_recording_does_not_have_and_a_track_that_does_not_exist() {
    // Arrange
    let directory = Temporary::new("export-refusals");
    let single = corpus()
        .into_iter()
        .find(|spec| spec.name == "rewritten")
        .unwrap();
    let clip = directory.path().join("single.mp4");
    build(&single).unwrap().write_to(&clip).unwrap();
    let out_path = directory.path().join("out.mp4");

    // Act
    let rear = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        out_path.to_str().unwrap(),
    ])
    .unwrap();
    let missing = run(&[
        "export",
        clip.to_str().unwrap(),
        "--tracks",
        "0,7",
        "--out",
        out_path.to_str().unwrap(),
    ])
    .unwrap();

    // Assert
    assert_eq!(rear.status.code(), Some(2));
    assert!(err(&rear).contains("no rear camera"), "{}", err(&rear));
    assert_eq!(missing.status.code(), Some(2));
    assert!(err(&missing).contains("no track 7"), "{}", err(&missing));
    assert!(!out_path.exists());
    assert!(!directory.path().join("out.part").exists());
}

#[test]
fn export_joins_two_recordings_into_one_two_track_file() {
    // Arrange
    let directory = Temporary::new("export-join");
    let front_file = write_reference_like(directory.path(), "20260604_122323_F.mp4").unwrap();
    let rear_file = write_reference_like(directory.path(), "20260604_122323_R.mp4").unwrap();
    let joined = directory.path().join("joined.mp4");

    // Act
    let output = run(&[
        "export",
        front_file.to_str().unwrap(),
        rear_file.to_str().unwrap(),
        "--out",
        joined.to_str().unwrap(),
        "--format",
        "json",
    ])
    .unwrap();

    // Assert
    assert_eq!(output.status.code(), Some(0), "{}", err(&output));
    let json: serde_json::Value = serde_json::from_str(&out(&output)).unwrap();
    assert_eq!(
        json["manifest"]["produced"]["operation"]["class"],
        "two_track_archive"
    );
    assert_eq!(
        json["manifest"]["produced"]["operation"]["sources"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(json["report"]["renumbered"], true);
    assert_eq!(json["report"]["tracks"].as_array().unwrap().len(), 3);
    assert_eq!(json["manifest"]["tracks"].as_array().unwrap().len(), 3);
    let inspected = run(&["inspect", joined.to_str().unwrap()]).unwrap();
    assert!(out(&inspected).contains("3 track(s)"), "{}", out(&inspected));
}

#[test]
fn export_refuses_to_write_over_a_recording_it_reads_under_any_spelling_of_the_path() {
    // Arrange
    let directory = Temporary::new("export-overwrite");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let bytes_before = std::fs::read(&clip).unwrap();
    let modified_before = std::fs::metadata(&clip).unwrap().modified().unwrap();
    let relative = directory
        .path()
        .join("..")
        .join(directory.path().file_name().unwrap())
        .join("20260604_122323E.MP4");
    let elsewhere = directory.path().join("out.mp4");

    // Act
    let as_output = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        relative.to_str().unwrap(),
    ])
    .unwrap();
    let as_manifest = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        elsewhere.to_str().unwrap(),
        "--manifest",
        relative.to_str().unwrap(),
    ])
    .unwrap();

    // Assert
    assert_eq!(as_output.status.code(), Some(2));
    assert!(
        err(&as_output).contains("nothing was written"),
        "{}",
        err(&as_output)
    );
    assert_eq!(as_manifest.status.code(), Some(2));
    assert!(
        err(&as_manifest).contains("nothing was written"),
        "{}",
        err(&as_manifest)
    );
    assert_eq!(std::fs::read(&clip).unwrap(), bytes_before);
    assert_eq!(
        std::fs::metadata(&clip).unwrap().modified().unwrap(),
        modified_before
    );
    assert!(!elsewhere.exists());
    assert!(!directory.path().join("20260604_122323E.part").exists());
    assert!(!directory.path().join("out.part").exists());
}

#[test]
fn export_says_how_many_track_references_it_left_out() {
    // Arrange
    let directory = Temporary::new("export-references");
    let spec = corpus()
        .into_iter()
        .find(|spec| spec.quirks.track_references)
        .unwrap();
    let clip = directory.path().join("20260604_122323E.MP4");
    build(&spec).unwrap().write_to(&clip).unwrap();
    let out_path = directory.path().join("rear.mp4");

    // Act
    let output = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        out_path.to_str().unwrap(),
    ])
    .unwrap();
    let as_json = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "both",
        "--out",
        directory.path().join("both.mp4").to_str().unwrap(),
        "--format",
        "json",
    ])
    .unwrap();

    // Assert
    assert_eq!(output.status.code(), Some(0), "{}", err(&output));
    assert!(
        out(&output).contains("1 track reference(s) named a track this file does not hold"),
        "{}",
        out(&output)
    );
    assert_eq!(as_json.status.code(), Some(0), "{}", err(&as_json));
    let json: serde_json::Value = serde_json::from_str(&out(&as_json)).unwrap();
    assert_eq!(json["report"]["references_dropped"], 0);
}

#[test]
fn export_signs_last_and_verify_reads_the_credentials_back() {
    // Arrange
    let directory = Temporary::new("export-sign");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let out_path = directory.path().join("rear.mp4");

    // Act
    let exported = run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        out_path.to_str().unwrap(),
        "--sign",
    ])
    .unwrap();
    let verified = run(&[
        "verify",
        directory.path().join("rear.mp4.manifest.json").to_str().unwrap(),
        out_path.to_str().unwrap(),
        "--format",
        "json",
    ])
    .unwrap();
    let described = run(&[
        "verify",
        directory.path().join("rear.mp4.manifest.json").to_str().unwrap(),
        out_path.to_str().unwrap(),
    ])
    .unwrap();

    // Assert
    assert_eq!(exported.status.code(), Some(0), "{}", err(&exported));
    let printed = out(&exported);
    assert!(
        printed.contains("Content Credentials embedded in the file"),
        "{printed}"
    );
    assert!(printed.contains("on no trust list"), "{printed}");
    assert!(
        printed.contains("The signature carries no time stamp"),
        "{printed}"
    );
    assert!(
        printed.contains("Actions recorded: c2pa.opened, c2pa.repackaged"),
        "{printed}"
    );
    assert!(
        printed.contains("every track fingerprint in them is the file's"),
        "{printed}"
    );
    assert_eq!(verified.status.code(), Some(0), "{}", out(&verified));
    let json: serde_json::Value = serde_json::from_str(&out(&verified)).unwrap();
    assert_eq!(
        json["credentials"]["credentials"]["state"],
        "valid_not_on_trust_list"
    );
    assert_eq!(json["credentials"]["credentials"]["placement"], "embedded");
    assert_eq!(json["credentials"]["tracks_differing"], 0);
    assert_eq!(json["token"], serde_json::Value::Null);
    assert!(
        json["comparison"]["findings"]
            .as_array()
            .unwrap()
            .iter()
            .all(|finding| finding["outcome"] == "identical"),
        "{}",
        json["comparison"]
    );
    assert_eq!(described.status.code(), Some(0), "{}", out(&described));
    assert!(
        out(&described).contains("The signer's certificate chains to no trust list"),
        "{}",
        out(&described)
    );
}

#[test]
fn sign_writes_credentials_beside_a_recording_and_leaves_it_as_it_was() {
    // Arrange
    let directory = Temporary::new("sign-beside");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let bytes_before = std::fs::read(&clip).unwrap();
    let manifest = directory.path().join("clip.manifest.json");
    run(&[
        "fingerprint",
        clip.to_str().unwrap(),
        "--manifest",
        manifest.to_str().unwrap(),
    ])
    .unwrap();

    // Act
    let signed = run(&["sign", clip.to_str().unwrap()]).unwrap();
    let verified = run(&["verify", manifest.to_str().unwrap(), clip.to_str().unwrap()]).unwrap();

    // Assert
    assert_eq!(signed.status.code(), Some(0), "{}", err(&signed));
    let sidecar = directory.path().join("20260604_122323E.c2pa");
    assert!(sidecar.is_file());
    assert!(
        out(&signed).contains("Content Credentials written beside the file"),
        "{}",
        out(&signed)
    );
    assert_eq!(std::fs::read(&clip).unwrap(), bytes_before);
    assert_eq!(verified.status.code(), Some(0), "{}", out(&verified));
    assert!(
        out(&verified).contains("Content Credentials beside the file"),
        "{}",
        out(&verified)
    );
    assert!(
        out(&verified).contains("Actions recorded: c2pa.opened"),
        "{}",
        out(&verified)
    );
}

#[test]
fn verify_says_when_a_file_carries_no_credentials_and_reports_a_broken_binding() {
    // Arrange
    let directory = Temporary::new("verify-credentials");
    let clip = write_reference_like(directory.path(), "20260604_122323E.MP4").unwrap();
    let signed_path = directory.path().join("rear.mp4");
    run(&[
        "export",
        clip.to_str().unwrap(),
        "--camera",
        "rear",
        "--out",
        signed_path.to_str().unwrap(),
        "--sign",
    ])
    .unwrap();
    let signed_manifest = directory.path().join("rear.mp4.manifest.json");
    // A rewrite of the signed file that keeps every sample and moves the boxes.
    let rewritten = directory.path().join("rewritten.mp4");
    run(&[
        "export",
        signed_path.to_str().unwrap(),
        "--camera",
        "both",
        "--out",
        rewritten.to_str().unwrap(),
    ])
    .unwrap();
    let clip_manifest = directory.path().join("clip.manifest.json");
    run(&[
        "fingerprint",
        clip.to_str().unwrap(),
        "--manifest",
        clip_manifest.to_str().unwrap(),
    ])
    .unwrap();

    // Act
    let none = run(&["verify", clip_manifest.to_str().unwrap(), clip.to_str().unwrap()]).unwrap();
    let broken = run(&[
        "verify",
        signed_manifest.to_str().unwrap(),
        rewritten.to_str().unwrap(),
    ])
    .unwrap();

    // Assert
    assert_eq!(none.status.code(), Some(0), "{}", out(&none));
    assert!(
        out(&none).contains("No Content Credentials are embedded in the file or beside it"),
        "{}",
        out(&none)
    );
    assert_eq!(broken.status.code(), Some(1), "{}", out(&broken));
    assert!(
        out(&broken).contains("does not hold: ") && out(&broken).contains("assertion.bmffHash.mismatch"),
        "{}",
        out(&broken)
    );
}
