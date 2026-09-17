//! Grouping files into recordings.
//!
//! A recording is one moment seen by one recorder: one file holding every camera, or one file per camera
//! that share a stem. The player, the archive and the manifest accept either as one source, which is what
//! the product's name promises: to join.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::derivative::{Origin, assess};
use crate::grammar::Camera;
use crate::probe::Probe;

/// One file that was probed, with where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProbedFile {
    /// The path as given.
    pub path: PathBuf,
    /// What the probe learned.
    pub probe: Probe,
}

/// One file of a paired recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Member {
    /// Which camera the file holds, by its name.
    pub camera: Camera,
    /// The file.
    pub file: ProbedFile,
    /// What its structure shows about its origin.
    pub origin: Origin,
}

/// One recording, or one file that could not be placed in a recording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Recording {
    /// One file holding the cameras it holds.
    SingleFile {
        /// The file.
        file: ProbedFile,
        /// How many video tracks it holds.
        video_tracks: u32,
        /// What its structure shows about its origin.
        origin: Origin,
    },
    /// Several files, one per camera, sharing a stem.
    Paired {
        /// The stem they share.
        stem: String,
        /// The files, front first, then rear, then any other camera.
        members: Vec<Member>,
    },
    /// A file named as one camera of a pair whose counterpart was not found.
    Unpaired {
        /// The file.
        file: ProbedFile,
        /// Which camera its name says it holds.
        camera: Camera,
        /// What its structure shows about its origin.
        origin: Origin,
    },
    /// A file that is a container and fits no naming grammar.
    Unrecognised {
        /// The file.
        file: ProbedFile,
    },
    /// A file that could not be read as a container.
    Unreadable {
        /// The file.
        file: ProbedFile,
        /// Why.
        reason: String,
    },
}

/// Groups probed files into recordings.
///
/// Files are grouped by grammar and stem. A stem with a front file and a rear file is a pair; a stem with
/// one camera file is unpaired; a file whose name says it holds every camera stands alone. Files that
/// fit no grammar but read as containers are reported as unrecognised, and files that do not read at all
/// as unreadable, so that nothing given to the scan disappears from its answer.
#[must_use]
pub fn group(files: Vec<ProbedFile>) -> Vec<Recording> {
    let siblings: Vec<Probe> = files.iter().map(|file| file.probe.clone()).collect();
    let mut recordings = Vec::new();
    let mut stems: BTreeMap<String, Vec<ProbedFile>> = BTreeMap::new();

    for file in files {
        if let Some(reason) = file.probe.unreadable.clone() {
            recordings.push(Recording::Unreadable { file, reason });
            continue;
        }
        match file
            .probe
            .parsed
            .as_ref()
            .map(|parsed| (parsed.camera, parsed.stem.clone()))
        {
            None => recordings.push(Recording::Unrecognised { file }),
            Some((Camera::All, _)) => {
                let origin = assess(&file.probe, &siblings);
                let video_tracks = file
                    .probe
                    .container
                    .as_ref()
                    .map_or(0, |summary| summary.video_tracks);
                recordings.push(Recording::SingleFile {
                    file,
                    video_tracks,
                    origin,
                });
            },
            Some((_, stem)) => stems.entry(stem).or_default().push(file),
        }
    }

    for (stem, files) in stems {
        let mut members: Vec<Member> = files
            .into_iter()
            .map(|file| Member {
                camera: file
                    .probe
                    .parsed
                    .as_ref()
                    .map_or(Camera::Front, |parsed| parsed.camera),
                origin: assess(&file.probe, &siblings),
                file,
            })
            .collect();
        members.sort_by_key(|member| match member.camera {
            Camera::Front => 0,
            Camera::Rear => 1,
            Camera::Interior => 2,
            Camera::All => 3,
        });

        if members.len() == 1
            && let Some(single) = members.pop()
        {
            recordings.push(Recording::Unpaired {
                file: single.file,
                camera: single.camera,
                origin: single.origin,
            });
            continue;
        }

        recordings.push(Recording::Paired { stem, members });
    }

    recordings
}
