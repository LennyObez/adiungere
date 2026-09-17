//! **G23.** The sample tables an export writes are consistent, and an independent parser reads the same
//! number of samples from them as the product wrote.
//!
//! A container whose tables only this product can read is a container nobody else can check. So every
//! export mode of every corpus recording is written to disk and handed to the pinned media tool, which
//! counts the packets of every stream and decodes every one of them; the counts have to equal the sample
//! counts the product reports, and the decode has to finish without a word on its error stream. The tool
//! is fetched by its digest and never linked; when it is absent this guarantee fails rather than skips.

use std::path::{Path, PathBuf};
use std::process::Command;

use adiungere_fixtures::{build, corpus};
use adiungere_guarantees::{pinned_media_prober, pinned_media_tool};
use adiungere_isobmff::{Input, RemuxPlan, SliceSource, TrackKind, parse, remux};

/// A directory that exists for one test and is removed when the test ends, whichever way it ends.
struct Temporary(PathBuf);

impl Temporary {
    fn new(name: &str) -> Self {
        let unique = format!(
            "adiungere-g23-{name}-{}-{:?}",
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

/// Writes one export and returns the sample count of every output track, in order.
fn export_to(bytes: &[u8], tracks: &[usize], path: &Path) -> Result<Vec<u32>, adiungere_isobmff::Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: tracks.to_vec(),
    }];
    let mut file = std::fs::File::create(path)?;
    let report = remux(&mut inputs, &RemuxPlan::default(), &mut file, &mut |_| true)?;
    Ok(report.tracks.iter().map(|track| track.samples).collect())
}

/// The packet count of every stream as the prober reports it, in stream order.
fn packets_counted_by_the_prober(prober: &Path, path: &Path) -> Result<Vec<u32>, String> {
    let output = Command::new(prober)
        .args([
            "-v",
            "error",
            "-count_packets",
            "-show_entries",
            "stream=nb_read_packets",
            "-of",
            "csv=p=0",
        ])
        .arg(path)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| {
            line.trim()
                .trim_end_matches(',')
                .parse::<u32>()
                .map_err(|e| e.to_string())
        })
        .collect()
}

/// What the tool writes on its error stream while decoding every stream of the file; empty when nothing
/// went wrong.
fn decode_complaints(tool: &Path, path: &Path) -> Result<String, String> {
    let output = Command::new(tool)
        .args(["-v", "error", "-i"])
        .arg(path)
        .args(["-f", "null", "-"])
        .output()
        .map_err(|error| error.to_string())?;
    Ok(String::from_utf8_lossy(&output.stderr).trim().to_owned())
}

#[test]
fn the_pinned_tool_counts_the_same_samples_and_decodes_every_export_without_complaint() {
    // Arrange
    let tool = pinned_media_tool().unwrap();
    let prober = pinned_media_prober().unwrap();
    let directory = Temporary::new("exports");
    let mut checked = 0usize;

    for spec in corpus() {
        let Some(original) = build(&spec).unwrap().bytes() else {
            continue;
        };
        let source = parse(&mut SliceSource::new(&original)).unwrap();
        let all = source.tracks().unwrap();
        let videos: Vec<usize> = all
            .iter()
            .filter(|track| track.kind == TrackKind::Video)
            .map(|track| track.index)
            .collect();
        let audios: Vec<usize> = all
            .iter()
            .filter(|track| track.kind != TrackKind::Video)
            .map(|track| track.index)
            .collect();
        let mut modes: Vec<Vec<usize>> = vec![(0..all.len()).collect()];
        for video in &videos {
            let mut tracks = vec![*video];
            tracks.extend(audios.iter().copied());
            tracks.sort_unstable();
            modes.push(tracks);
        }

        for (mode, tracks) in modes.iter().enumerate() {
            // Act
            let path = directory.0.join(format!("{}-{mode}.mp4", spec.name));
            let written = export_to(&original, tracks, &path).unwrap();
            let counted = packets_counted_by_the_prober(&prober, &path).unwrap();
            let complaints = decode_complaints(&tool, &path).unwrap();

            // Assert
            assert_eq!(
                counted, written,
                "{} {tracks:?}: the prober's packet counts",
                spec.name
            );
            assert!(
                complaints.is_empty(),
                "{} {tracks:?}: the tool complained while decoding:\n{complaints}",
                spec.name
            );
            checked += 1;
        }
    }

    assert!(
        checked >= 25,
        "only {checked} exports were checked; this check is inert"
    );
}

#[test]
fn the_tool_would_complain_about_a_table_that_points_past_the_media() {
    // A detection test: the same decode check catches an export whose media was cut short.

    // Arrange
    let tool = pinned_media_tool().unwrap();
    let directory = Temporary::new("cut");
    let original = build(&corpus().into_iter().next().unwrap())
        .unwrap()
        .bytes()
        .unwrap();
    let path = directory.0.join("whole.mp4");
    export_to(&original, &[0, 1, 2], &path).unwrap();
    let whole = std::fs::read(&path).unwrap();
    let cut = directory.0.join("cut.mp4");
    std::fs::write(&cut, &whole[..whole.len() - 2000]).unwrap();

    // Act
    let complaints = decode_complaints(&tool, &cut).unwrap();

    // Assert
    assert!(
        !complaints.is_empty(),
        "the tool decoded a truncated file in silence"
    );
}
