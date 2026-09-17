//! A cache of probes, keyed by what changes when a file changes.
//!
//! A library of recordings is scanned again and again; a file that has the same size and modification
//! time as last time is the same file for the purpose of a probe. The cache is a plain document a person
//! can read and delete.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::probe::Probe;

/// One cached probe with the facts it is keyed by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// The file size when probed.
    pub size: u64,
    /// The modification time when probed, in seconds since the epoch.
    pub modified: i64,
    /// The probe.
    pub probe: Probe,
}

/// The cache.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScanCache {
    /// The format identifier.
    pub format: String,
    /// Entries by path, as the path was given.
    pub entries: BTreeMap<String, Entry>,
}

/// The identifier of this cache format.
pub const CACHE_FORMAT: &str = "adiungere-scan-cache/1";

impl ScanCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            format: CACHE_FORMAT.to_owned(),
            entries: BTreeMap::new(),
        }
    }

    /// Reads a cache from a file, or starts empty when the file does not exist. A cache whose format is
    /// unknown or whose text does not parse is discarded rather than trusted.
    ///
    /// # Errors
    ///
    /// Returns an error when the file exists and cannot be read.
    pub fn load(path: &Path) -> Result<Self, std::io::Error> {
        if !path.exists() {
            return Ok(Self::new());
        }
        let text = std::fs::read_to_string(path)?;
        let cache: Self = match serde_json::from_str(&text) {
            Ok(cache) => cache,
            Err(_) => return Ok(Self::new()),
        };
        if cache.format != CACHE_FORMAT {
            return Ok(Self::new());
        }
        Ok(cache)
    }

    /// Writes the cache to a file.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be written.
    pub fn save(&self, path: &Path) -> Result<(), std::io::Error> {
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        std::fs::write(path, text)
    }

    /// The cached probe for a path, when the size and modification time still match.
    #[must_use]
    pub fn get(&self, path: &str, size: u64, modified: i64) -> Option<&Probe> {
        self.entries
            .get(path)
            .filter(|entry| entry.size == size && entry.modified == modified)
            .map(|entry| &entry.probe)
    }

    /// Records a probe.
    pub fn insert(&mut self, path: &str, size: u64, modified: i64, probe: Probe) {
        self.entries.insert(
            path.to_owned(),
            Entry {
                size,
                modified,
                probe,
            },
        );
    }
}

/// The modification time of a file in seconds since the epoch, or zero when the platform cannot say.
#[must_use]
pub fn modified_seconds(metadata: &std::fs::Metadata) -> i64 {
    metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |elapsed| i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    // The unit tests here touch no file and no clock, so that they run under the undefined-behaviour
    // interpreter, which isolates both; the tests over a real file system are with the integration tests.

    use super::ScanCache;
    use crate::probe::Probe;

    fn probe(name: &str) -> Probe {
        Probe {
            name: name.to_owned(),
            size: 0,
            parsed: None,
            container: None,
            unreadable: None,
            bytes_read: 0,
        }
    }

    #[test]
    fn a_cached_probe_is_returned_only_while_the_size_and_the_modification_time_both_match() {
        // Arrange
        let mut cache = ScanCache::new();
        cache.insert("a.mp4", 100, 1_700_000_000, probe("a.mp4"));

        // Act and assert
        assert!(cache.get("a.mp4", 100, 1_700_000_000).is_some());
        assert!(cache.get("a.mp4", 101, 1_700_000_000).is_none());
        assert!(cache.get("a.mp4", 100, 1_700_000_001).is_none());
        assert!(cache.get("a.mp4", 101, 1_700_000_001).is_none());
        assert!(cache.get("b.mp4", 100, 1_700_000_000).is_none());
    }
}
