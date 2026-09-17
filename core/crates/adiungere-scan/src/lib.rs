//! Finding recordings.
//!
//! A gallery holds thousands of files and a dashcam's are among them, named by a grammar and shaped by a
//! recorder. This crate reads names under the grammars it knows, probes each file just far enough to say
//! what it holds, groups files into recordings, including the pairs of recorders that write one file per
//! camera, and notes the signs that a file was rewritten by another program on its way from the memory
//! card. Probes are cached by size and modification time so a library is not read twice.

#![forbid(unsafe_code)]

pub mod cache;
pub mod derivative;
pub mod grammar;
pub mod probe;
pub mod recording;

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub use cache::{CACHE_FORMAT, ScanCache};
pub use derivative::{Origin, Signal, assess};
pub use grammar::{Camera, Event, Grammar, ParsedName, parse_name};
pub use probe::{ContainerSummary, Probe, probe_file, probe_source};
pub use recording::{Member, ProbedFile, Recording, group};

/// The extensions a recording may carry, compared without regard to case. A file with any other
/// extension is skipped without being opened, unless its name fits a grammar, because a library holds a
/// hundred thousand photographs for every recording and a scan that opened each one would take the
/// afternoon the bounded probe was designed to save.
pub const RECORDING_EXTENSIONS: [&str; 5] = ["mp4", "mov", "m4v", "3gp", "3g2"];

/// What a scan found, and what it did not look at.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scan {
    /// Every recording, and every file that was opened and could not be placed in one.
    pub recordings: Vec<Recording>,
    /// How many files were skipped by their extension without being opened.
    pub skipped_by_extension: u64,
}

/// Whether a file is worth opening: a recording's extension, or a name that fits a grammar.
#[must_use]
pub fn worth_opening(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let extension = path
        .extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    RECORDING_EXTENSIONS.contains(&extension.as_str()) || parse_name(&name).is_some()
}

/// Scans paths: each file as given, each directory recursively, through a cache when one is given.
///
/// A path that is opened and cannot be read is reported as an unreadable recording rather than dropped,
/// so the answer accounts for everything it opened; what it did not open is counted.
#[must_use]
pub fn scan(paths: &[PathBuf], cache: Option<&mut ScanCache>) -> Scan {
    let mut files = Vec::new();
    for path in paths {
        collect(path, &mut files);
    }
    files.sort();

    let mut cache = cache;
    let mut probed = Vec::with_capacity(files.len());
    let mut skipped_by_extension = 0u64;

    for path in files {
        if !worth_opening(&path) {
            skipped_by_extension = skipped_by_extension.saturating_add(1);
            continue;
        }
        let key = path.to_string_lossy().into_owned();
        let metadata = std::fs::metadata(&path).ok();
        let size = metadata.as_ref().map_or(0, std::fs::Metadata::len);
        let modified = metadata.as_ref().map_or(0, cache::modified_seconds);

        let cached = cache
            .as_deref()
            .and_then(|cache| cache.get(&key, size, modified).cloned());
        let probe = match cached {
            Some(probe) => probe,
            None => match probe_file(&path) {
                Ok(probe) => {
                    if let Some(cache) = cache.as_deref_mut() {
                        cache.insert(&key, size, modified, probe.clone());
                    }
                    probe
                },
                Err(error) => Probe {
                    name: path
                        .file_name()
                        .map(|name| name.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    size,
                    parsed: None,
                    container: None,
                    unreadable: Some(error.to_string()),
                    bytes_read: 0,
                },
            },
        };
        probed.push(ProbedFile { path, probe });
    }

    Scan {
        recordings: group(probed),
        skipped_by_extension,
    }
}

fn collect(path: &Path, into: &mut Vec<PathBuf>) {
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                collect(&entry.path(), into);
            }
        }
    } else {
        into.push(path.to_path_buf());
    }
}
