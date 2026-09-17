//! The scanner over a directory of synthetic recordings named under each grammar.

use std::path::{Path, PathBuf};

use adiungere_fixtures::{Spec, build, corpus};
use adiungere_scan::{CACHE_FORMAT, Camera, Probe, Recording, ScanCache, Signal, scan};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-scan-{name}-{}-{:?}",
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

fn write(directory: &Path, name: &str, spec: &Spec) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let fixture = build(spec)?;
    let path = directory.join(name);
    fixture.write_to(&path)?;
    Ok(path)
}

fn spec_named(name: &str) -> Option<Spec> {
    corpus().into_iter().find(|spec| spec.name == name)
}

#[test]
fn a_directory_of_recordings_is_grouped_into_single_files_pairs_and_the_rest() {
    // Arrange
    let directory = Temporary::new("grouping");
    write(directory.path(), "20260604_122323E.MP4", &Spec::reference_like()).unwrap();
    write(directory.path(), "20260604_122423N.MP4", &Spec::reference_like()).unwrap();
    write(
        directory.path(),
        "20260101_101010_F.mp4",
        &spec_named("rewritten").unwrap(),
    )
    .unwrap();
    write(
        directory.path(),
        "20260101_101010_R.mp4",
        &spec_named("rewritten").unwrap(),
    )
    .unwrap();
    write(
        directory.path(),
        "20260102_101010_F.mp4",
        &spec_named("rewritten").unwrap(),
    )
    .unwrap();
    write(directory.path(), "holiday.mp4", &Spec::reference_like()).unwrap();
    std::fs::write(directory.path().join("notes.mp4"), b"not a container at all").unwrap();
    std::fs::write(directory.path().join("photo.jpg"), b"not opened at all").unwrap();

    // Act
    let scanned = scan(&[directory.path().to_path_buf()], None);
    let recordings = scanned.recordings;

    // Assert
    assert_eq!(scanned.skipped_by_extension, 1);
    let singles = recordings
        .iter()
        .filter(|recording| matches!(recording, Recording::SingleFile { video_tracks: 2, .. }))
        .count();
    let pairs: Vec<&Recording> = recordings
        .iter()
        .filter(|recording| matches!(recording, Recording::Paired { .. }))
        .collect();
    let unpaired = recordings
        .iter()
        .filter(|recording| {
            matches!(
                recording,
                Recording::Unpaired {
                    camera: Camera::Front,
                    ..
                }
            )
        })
        .count();
    let unrecognised = recordings
        .iter()
        .filter(|recording| matches!(recording, Recording::Unrecognised { .. }))
        .count();
    let unreadable = recordings
        .iter()
        .filter(|recording| matches!(recording, Recording::Unreadable { .. }))
        .count();
    assert_eq!(singles, 2);
    assert_eq!(pairs.len(), 1);
    if let Recording::Paired { stem, members } = pairs.first().unwrap() {
        assert_eq!(stem, "20260101_101010");
        let cameras: Vec<Camera> = members.iter().map(|member| member.camera).collect();
        assert_eq!(cameras, vec![Camera::Front, Camera::Rear]);
    }
    assert_eq!(unpaired, 1);
    assert_eq!(unrecognised, 1);
    assert_eq!(unreadable, 1);
    assert_eq!(recordings.len(), 6);
}

#[test]
fn a_rewritten_file_beside_a_recorder_file_shows_every_sign_and_the_recorder_file_shows_none() {
    // Arrange
    let directory = Temporary::new("signs");
    write(directory.path(), "20260604_122323E.MP4", &Spec::reference_like()).unwrap();
    write(
        directory.path(),
        "20260604_122423E.MP4",
        &spec_named("rewritten").unwrap(),
    )
    .unwrap();

    // Act
    let recordings = scan(&[directory.path().to_path_buf()], None).recordings;

    // Assert
    let origin_of = |name: &str| {
        recordings
            .iter()
            .find_map(|recording| match recording {
                Recording::SingleFile { file, origin, .. } if file.probe.name == name => Some(origin.clone()),
                _ => None,
            })
            .unwrap()
    };
    let recorder = origin_of("20260604_122323E.MP4");
    let rewritten = origin_of("20260604_122423E.MP4");
    assert!(recorder.layout_known);
    assert!(recorder.signals.is_empty(), "{:?}", recorder.signals);
    assert_eq!(
        rewritten.signals,
        vec![
            Signal::MoovAfterMdat,
            Signal::SingleVideoTrackWhereSiblingHasTwo,
            Signal::VendorBoxesAbsent,
        ]
    );
}

#[test]
fn the_probe_reads_a_bounded_amount_and_the_cache_spares_the_second_scan() {
    // Arrange
    let directory = Temporary::new("cache");
    write(directory.path(), "20260604_122323E.MP4", &Spec::reference_like()).unwrap();
    let cache_path = directory.path().join("cache.json");
    let mut cache = ScanCache::new();

    // Act
    let first = scan(&[directory.path().to_path_buf()], Some(&mut cache)).recordings;
    cache.save(&cache_path).unwrap();
    let mut reloaded = ScanCache::load(&cache_path).unwrap();
    let second = scan(&[directory.path().to_path_buf()], Some(&mut reloaded)).recordings;

    // Assert
    let bytes_read = match first.first().unwrap() {
        Recording::SingleFile { file, .. } => file.probe.bytes_read,
        other => panic!("{other:?}"),
    };
    assert!(bytes_read > 0 && bytes_read < 256 * 1024, "{bytes_read}");
    assert_eq!(
        reloaded.entries.len(),
        1,
        "the recording was probed; the cache file was skipped by its extension"
    );
    let probes_equal = match (first.first().unwrap(), second.first().unwrap()) {
        (Recording::SingleFile { file: a, .. }, Recording::SingleFile { file: b, .. }) => a.probe == b.probe,
        _ => false,
    };
    assert!(probes_equal);
}

#[test]
fn a_cache_of_another_format_is_started_afresh_rather_than_trusted() {
    // Arrange
    let directory = Temporary::new("foreign-cache");
    let cache_path = directory.path().join("cache.json");
    std::fs::write(&cache_path, r#"{"format":"something-else/9","entries":{}}"#).unwrap();

    // Act
    let cache = ScanCache::load(&cache_path).unwrap();

    // Assert
    assert_eq!(cache.format, "adiungere-scan-cache/1");
    assert!(cache.entries.is_empty());
}

#[test]
fn a_cache_that_does_not_parse_or_does_not_exist_starts_empty_and_a_saved_one_reloads() {
    // Arrange
    let directory = Temporary::new("cache-files");
    let garbled = directory.path().join("garbled.json");
    std::fs::write(&garbled, "{ this is not json").unwrap();
    let absent = directory.path().join("absent.json");
    let saved = directory.path().join("saved.json");
    let mut cache = ScanCache::new();
    cache.insert(
        "a.mp4",
        100,
        7,
        Probe {
            name: "a.mp4".to_owned(),
            size: 100,
            parsed: None,
            container: None,
            unreadable: None,
            bytes_read: 0,
        },
    );

    // Act
    let from_garbled = ScanCache::load(&garbled).unwrap();
    let from_absent = ScanCache::load(&absent).unwrap();
    cache.save(&saved).unwrap();
    let reloaded = ScanCache::load(&saved).unwrap();

    // Assert
    assert_eq!(from_garbled, ScanCache::new());
    assert_eq!(from_absent, ScanCache::new());
    assert_eq!(from_absent.format, CACHE_FORMAT);
    assert_eq!(reloaded, cache);
    assert!(reloaded.get("a.mp4", 100, 7).is_some());
}

#[test]
fn the_cache_key_is_the_modification_time_the_file_carries() {
    // A file whose modification time is set by hand is probed, and the cache entry carries that time.

    // Arrange
    let directory = Temporary::new("mtime");
    let path = write(directory.path(), "20260604_122323E.MP4", &Spec::reference_like()).unwrap();
    let stamp = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_modified(stamp)
        .unwrap();
    let mut cache = ScanCache::new();

    // Act
    let scanned = scan(&[directory.path().to_path_buf()], Some(&mut cache));

    // Assert
    assert_eq!(scanned.recordings.len(), 1);
    let entry = cache.entries.values().next().unwrap();
    assert_eq!(cache.entries.len(), 1);
    assert_eq!(entry.modified, 1_600_000_000);
    assert_eq!(entry.size, std::fs::metadata(&path).unwrap().len());
}
