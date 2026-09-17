//! **G14.** The production fingerprint equals the reference script on the corpus, and the elementary
//! stream digest equals what the pinned media tool produces.
//!
//! The fingerprint is only worth something if a stranger can reproduce it without this code. So two
//! independent implementations are run on every corpus recording and every track: the reference script in
//! `scripts/track-fingerprint.py`, which uses nothing but an interpreter's standard library, and the
//! common media tool at the version pinned in `tools/versions.toml`, whose elementary stream output is the
//! definition of the secondary digest. A missing interpreter or a missing tool fails this guarantee rather
//! than skipping it: a step that did not run is not a step that passed.

use std::path::{Path, PathBuf};
use std::process::Command;

use adiungere_fingerprint::{Digest, annex_b_digest, track_fingerprint};
use adiungere_fixtures::{build, corpus};
use adiungere_guarantees::{read_at, repository_root, without_hash_comments};
use adiungere_isobmff::{TrackKind, parse};
use sha2::{Digest as _, Sha256};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g14-{name}-{}-{:?}",
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
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The value of a key inside one table of the versions file.
fn pinned(table: &str, key: &str) -> Option<String> {
    let source = without_hash_comments(&read_at("tools/versions.toml").ok()?);
    let mut inside = false;
    for line in source.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == format!("[{table}]");
            continue;
        }
        if inside
            && let Some((name, value)) = line.split_once('=')
            && name.trim() == key
        {
            return Some(value.trim().trim_matches('"').to_owned());
        }
    }
    None
}

/// Whether a candidate media tool sits where the versions file says the fetched one is, or on the path,
/// and reports the pinned version. The versions file decides acceptance and nothing else: no value read
/// from it becomes part of a command.
fn is_the_pinned_ffmpeg(candidate: &Path, tools: &Path) -> bool {
    let Some(version) = pinned("ffmpeg", "version") else {
        return false;
    };
    let Some(binary) = pinned("ffmpeg.linux-x86_64", "binary") else {
        return false;
    };
    if candidate.starts_with(tools) && candidate != tools.join(binary) {
        return false;
    }
    let Ok(output) = Command::new(candidate).arg("-version").output() else {
        return false;
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .is_some_and(|first| first.contains(&version))
}

/// The pinned media tool, from the fetched tools directory or the path, at the pinned version only.
///
/// The candidates come from listing the tools directory, never from a value read out of a file.
fn pinned_ffmpeg() -> Result<PathBuf, String> {
    let tools = repository_root().join(".tools");
    let mut candidates: Vec<PathBuf> = std::fs::read_dir(&tools)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path().join("bin").join("ffmpeg"))
                .collect()
        })
        .unwrap_or_default();
    candidates.push(PathBuf::from("ffmpeg"));

    candidates
        .into_iter()
        .find(|candidate| is_the_pinned_ffmpeg(candidate, &tools))
        .ok_or_else(|| {
            "the media tool at the version pinned in tools/versions.toml was not found. Run \
             scripts/fetch-tools.sh; a guarantee that skipped it would be a guarantee that proved nothing"
                .to_owned()
        })
}

/// Whether a candidate interpreter is on the pinned release line.
fn is_the_pinned_python(candidate: &str) -> bool {
    let Some(line) = pinned("python", "version") else {
        return false;
    };
    let Ok(output) = Command::new(candidate).arg("--version").output() else {
        return false;
    };
    let text = String::from_utf8_lossy(&output.stdout).to_string() + &String::from_utf8_lossy(&output.stderr);
    text.contains(&format!("Python {line}."))
}

fn python() -> Result<PathBuf, String> {
    ["python3", "python"]
        .into_iter()
        .find(|candidate| is_the_pinned_python(candidate))
        .map(PathBuf::from)
        .ok_or_else(|| {
            "no interpreter on the release line pinned in tools/versions.toml was found; the reference \
             script cannot be compared"
                .to_owned()
        })
}

/// Writes every non-sparse corpus recording into a directory, or reports the first failure.
fn write_corpus(into: &Path) -> Result<Vec<(String, PathBuf)>, Box<dyn std::error::Error>> {
    let mut written = Vec::new();
    for spec in corpus().into_iter().filter(|spec| !spec.layout.sparse) {
        let path = into.join(format!("{}.mp4", spec.name));
        build(&spec)?.write_to(&path)?;
        written.push((spec.name.to_owned(), path));
    }
    Ok(written)
}

#[test]
fn the_reference_script_reproduces_every_fingerprint_on_the_corpus() {
    // Arrange
    let interpreter = python().unwrap();
    let script = repository_root().join("scripts/track-fingerprint.py");
    let directory = Temporary::new("script");
    let recordings = write_corpus(&directory.0).unwrap();
    let mut compared = 0usize;

    for (name, path) in &recordings {
        let mut source = adiungere_isobmff::FileSource::open(path).unwrap();
        let container = parse(&mut source).unwrap();
        // The script numbers tracks by their position under the movie box, which is what the reader's
        // index is; the position counted here is what the script is given, and the two are checked equal.
        for (position, track) in container.tracks().unwrap().into_iter().enumerate() {
            assert_eq!(
                track.index, position,
                "{name}: the reader's track index is its position"
            );
            let ours = track_fingerprint(&mut source, &container, &track).unwrap();
            let stream = (track.kind == TrackKind::Video)
                .then(|| annex_b_digest(&mut source, &container, &track).unwrap());

            // Act
            let mut command = Command::new(&interpreter);
            command.arg(&script).arg(path).arg(position.to_string());
            if stream.is_some() {
                command.arg("--annexb");
            }
            let output = command.output().unwrap();
            let printed = String::from_utf8_lossy(&output.stdout).into_owned();

            // Assert
            assert!(
                output.status.success(),
                "{name} track {position}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(
                printed.contains(&format!("payload {}", ours.payload_sha256)),
                "{name} track {position}: the script's payload digest differs:\n{printed}"
            );
            assert!(
                printed.contains(&format!("configuration {}", ours.configuration_sha256)),
                "{name} track {position}: the script's configuration digest differs:\n{printed}"
            );
            if let Some(stream) = stream {
                assert!(
                    printed.contains(&format!("stream {}", stream.sha256)),
                    "{name} track {position}: the script's stream digest differs:\n{printed}"
                );
            }
            compared += 1;
        }
    }

    assert!(
        compared >= 20,
        "only {compared} tracks were compared; this check is inert"
    );
}

#[test]
fn the_pinned_media_tool_reproduces_every_elementary_stream_digest_on_the_corpus() {
    // Arrange
    let tool = pinned_ffmpeg().unwrap();
    let directory = Temporary::new("tool");
    let recordings = write_corpus(&directory.0).unwrap();
    let mut compared = 0usize;

    for (name, path) in &recordings {
        let mut source = adiungere_isobmff::FileSource::open(path).unwrap();
        let container = parse(&mut source).unwrap();
        let video: Vec<_> = container
            .tracks()
            .unwrap()
            .into_iter()
            .filter(|track| track.kind == TrackKind::Video)
            .collect();

        for (video_index, track) in video.iter().enumerate() {
            let ours = annex_b_digest(&mut source, &container, track).unwrap();

            // Act
            let output = Command::new(&tool)
                .args(["-v", "error", "-i"])
                .arg(path)
                .args([
                    "-map",
                    &format!("0:v:{video_index}"),
                    "-c",
                    "copy",
                    "-f",
                    "h264",
                    "-",
                ])
                .output()
                .unwrap();
            let theirs = Digest(Sha256::digest(&output.stdout).into());

            // Assert
            assert!(
                output.status.success(),
                "{name}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                ours.sha256, theirs,
                "{name} video track {video_index}: the tool's stream differs from the rule"
            );
            assert_eq!(ours.stream_bytes, output.stdout.len() as u64);
            compared += 1;
        }
    }

    assert!(
        compared >= 12,
        "only {compared} streams were compared; this check is inert"
    );
}

#[test]
fn the_versions_file_pins_the_tool_and_the_interpreter_exactly() {
    // Act
    let tool = pinned("ffmpeg", "version").unwrap();
    let interpreter = pinned("python", "version").unwrap();
    let digest = pinned("ffmpeg.linux-x86_64", "sha256").unwrap();

    // Assert
    assert!(
        tool.starts_with('n') && tool.contains("-g"),
        "the tool pin in tools/versions.toml is not a build identifier"
    );
    assert_eq!(
        interpreter.split('.').count(),
        2,
        "the interpreter pin is a release line"
    );
    assert!(
        digest.parse::<Digest>().is_ok(),
        "the archive digest is not a SHA-256 digest"
    );
}
