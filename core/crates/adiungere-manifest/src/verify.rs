//! Comparing a manifest with a file: findings, one per subject, never a verdict.
//!
//! Each subject the manifest records is compared with what the file holds now, and the outcome is stated
//! for that subject alone. A whole-file digest that differs while every track is identical is a container
//! that was rewritten around unchanged samples; the findings say exactly that, and the reader draws the
//! conclusion, because that reader is the one who will be asked to defend it.

use adiungere_fingerprint::{Digest, file_digest, structural_fingerprint, track_fingerprint};
use adiungere_isobmff::{Container, Source, parse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::Error;
use crate::manifest::{Manifest, VendorLocation};

/// What was compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "subject", rename_all = "snake_case")]
pub enum Subject {
    /// Every byte of the file.
    WholeFile,
    /// One track's samples and decoder configuration.
    Track {
        /// The track, by index.
        index: usize,
    },
    /// One vendor box.
    VendorBox {
        /// Where it sits.
        location: VendorLocation,
        /// Its type code.
        kind: String,
    },
    /// The container's shape.
    Structure,
}

/// What the comparison found for one subject.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Outcome {
    /// The bytes are the same.
    Identical,
    /// The bytes differ.
    Differs {
        /// What the manifest records.
        recorded: Digest,
        /// What the file holds now.
        observed: Digest,
    },
    /// The manifest records nothing for this subject, so nothing was compared.
    NotRecorded,
    /// The manifest records the subject and the file does not have it.
    Missing,
    /// The file has the subject and the manifest does not record it.
    Unexpected,
    /// The subject could not be compared.
    NotCheckable {
        /// Why.
        reason: String,
    },
}

/// One finding.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Finding {
    /// What was compared.
    #[serde(flatten)]
    pub subject: Subject,
    /// What was found.
    #[serde(flatten)]
    pub outcome: Outcome,
}

/// Every finding of one comparison.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Verification {
    /// The findings, whole file first, then tracks, vendor boxes and structure.
    pub findings: Vec<Finding>,
}

impl Verification {
    /// Whether every subject that was compared came out identical, and at least one was compared.
    ///
    /// This is a fact about the findings, not a verdict about the recording: it says the bytes the manifest
    /// recorded are the bytes the file holds now.
    #[must_use]
    pub fn every_compared_subject_is_identical(&self) -> bool {
        let compared: Vec<&Finding> = self
            .findings
            .iter()
            .filter(|finding| !matches!(finding.outcome, Outcome::NotRecorded))
            .collect();
        !compared.is_empty()
            && compared
                .iter()
                .all(|finding| matches!(finding.outcome, Outcome::Identical))
    }
}

/// Compares a manifest with a file.
///
/// # Errors
///
/// Returns an error when the file cannot be read as a container at all. Anything less is a finding.
pub fn verify<S: Source>(manifest: &Manifest, source: &mut S) -> Result<Verification, Error> {
    let container = parse(source)?;
    let mut findings = Vec::new();

    findings.push(Finding {
        subject: Subject::WholeFile,
        outcome: match manifest.file.sha256 {
            None => Outcome::NotRecorded,
            Some(recorded) => {
                let (observed, _) = file_digest(source)?;
                compare(recorded, observed)
            },
        },
    });

    let tracks = container.tracks();
    for record in &manifest.tracks {
        let outcome = match (&record.fingerprint, &tracks) {
            (None, _) => Outcome::NotRecorded,
            (Some(_), Err(error)) => Outcome::NotCheckable {
                reason: error.to_string(),
            },
            (Some(recorded), Ok(tracks)) => match tracks.get(record.index) {
                None => Outcome::Missing,
                Some(track)
                    if track
                        .entry()
                        .is_none_or(|entry| entry.kind.to_string() != record.coding) =>
                {
                    Outcome::NotCheckable {
                        reason: format!(
                            "the track at this position is {}; the manifest records {}",
                            track.entry().map_or_else(
                                || "without a sample entry".to_owned(),
                                |entry| entry.kind.to_string()
                            ),
                            record.coding
                        ),
                    }
                },
                Some(track) => match track_fingerprint(source, &container, track) {
                    Err(error) => Outcome::NotCheckable {
                        reason: error.to_string(),
                    },
                    Ok(observed) => {
                        if observed.configuration_sha256 == recorded.configuration_sha256 {
                            compare(recorded.payload_sha256, observed.payload_sha256)
                        } else {
                            Outcome::Differs {
                                recorded: recorded.configuration_sha256,
                                observed: observed.configuration_sha256,
                            }
                        }
                    },
                },
            },
        };
        findings.push(Finding {
            subject: Subject::Track { index: record.index },
            outcome,
        });
    }

    findings.extend(vendor_findings(manifest, &container));

    let observed_shape = structural_fingerprint(&container).sha256;
    findings.push(Finding {
        subject: Subject::Structure,
        outcome: compare(
            manifest.file.structure.structural_fingerprint.sha256,
            observed_shape,
        ),
    });

    Ok(Verification { findings })
}

fn compare(recorded: Digest, observed: Digest) -> Outcome {
    if recorded == observed {
        Outcome::Identical
    } else {
        Outcome::Differs { recorded, observed }
    }
}

/// The vendor boxes in the file, as `(location, kind, digest)`, in file order.
fn present_vendor_boxes(container: &Container) -> Vec<(VendorLocation, String, Option<Digest>)> {
    let mut present: Vec<(u64, VendorLocation, String, Option<Digest>)> = container
        .udta_children()
        .into_iter()
        .map(|range| {
            (
                range.offset,
                VendorLocation::UserData,
                range.kind.to_string(),
                container.bytes_of(range).map(Digest::of),
            )
        })
        .chain(container.unknown_top_level().into_iter().map(|range| {
            (
                range.offset,
                VendorLocation::TopLevel,
                range.kind.to_string(),
                container.bytes_of(range).map(Digest::of),
            )
        }))
        .collect();
    present.sort_by_key(|(offset, _, _, _)| *offset);
    present
        .into_iter()
        .map(|(_, location, kind, digest)| (location, kind, digest))
        .collect()
}

fn vendor_findings(manifest: &Manifest, container: &Container) -> Vec<Finding> {
    let present = present_vendor_boxes(container);
    let mut matched = vec![false; present.len()];
    let mut findings = Vec::new();

    for recorded in &manifest.vendor.boxes {
        let position = present
            .iter()
            .enumerate()
            .position(|(index, (location, kind, _))| {
                !matched.get(index).copied().unwrap_or(true)
                    && *location == recorded.location
                    && *kind == recorded.kind
            });
        let outcome = match position {
            None => Outcome::Missing,
            Some(index) => {
                if let Some(slot) = matched.get_mut(index) {
                    *slot = true;
                }
                match (
                    recorded.sha256,
                    present.get(index).and_then(|(_, _, digest)| *digest),
                ) {
                    (None, _) => Outcome::NotRecorded,
                    (Some(_), None) => Outcome::NotCheckable {
                        reason: "the box is larger than the reader holds".to_owned(),
                    },
                    (Some(recorded), Some(observed)) => compare(recorded, observed),
                }
            },
        };
        findings.push(Finding {
            subject: Subject::VendorBox {
                location: recorded.location,
                kind: recorded.kind.clone(),
            },
            outcome,
        });
    }

    for (index, (location, kind, _)) in present.iter().enumerate() {
        if !matched.get(index).copied().unwrap_or(true) {
            findings.push(Finding {
                subject: Subject::VendorBox {
                    location: *location,
                    kind: kind.clone(),
                },
                outcome: Outcome::Unexpected,
            });
        }
    }

    findings
}
