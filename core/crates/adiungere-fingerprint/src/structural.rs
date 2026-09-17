//! The writer's structural fingerprint: the shape of the container, not its media.
//!
//! Two recordings from one device share a shape: the same brand, the same box order, the same encoder
//! tags, the same vendor boxes in the same places, the same timescales. A file that another program wrote
//! from that recording has a different shape even when its samples are identical. The fingerprint is the
//! digest of a canonical listing of those observations, and the observations are published beside it so a
//! reader can see what was compared rather than trust a number.

use adiungere_isobmff::{Container, TrackKind};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::digest::Digest;

/// The observations and their digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StructuralFingerprint {
    /// The observations, one per line, in the order they were digested.
    pub observations: Vec<String>,
    /// The digest of the observations joined by newlines.
    pub sha256: Digest,
}

/// Lists what the container's shape shows and digests the list.
#[must_use]
pub fn structural_fingerprint(container: &Container) -> StructuralFingerprint {
    let mut observations = Vec::new();

    if let Some(ftyp) = container.ftyp() {
        observations.push(format!("brand {}", ftyp.major_brand));
        let compatible: Vec<String> = ftyp.compatible_brands.iter().map(ToString::to_string).collect();
        observations.push(format!("compatible {}", compatible.join(" ")));
    } else {
        observations.push("brand none".to_owned());
    }

    let order: Vec<String> = container
        .top_level()
        .iter()
        .map(|range| range.kind.to_string())
        .collect();
    observations.push(format!("top-level {}", order.join(" ")));

    for range in container.unknown_top_level() {
        observations.push(format!("top-level-unknown {} {}", range.kind, range.size));
    }

    if let Some(tail) = container.clipped_tail() {
        observations.push(format!("top-level-clipped {} {}", tail.kind, tail.size));
    }

    if let Ok(mvhd) = container.mvhd() {
        observations.push(format!("movie-timescale {}", mvhd.timescale));
    }

    let moov: Vec<String> = container
        .moov()
        .children
        .iter()
        .map(|child| child.kind.to_string())
        .collect();
    observations.push(format!("moov {}", moov.join(" ")));

    for range in container.udta_children() {
        observations.push(format!("udta {} {}", range.kind, range.size));
    }

    if let Ok(tracks) = container.tracks() {
        for track in &tracks {
            let kind = match track.kind {
                TrackKind::Video => "video".to_owned(),
                TrackKind::Audio => "audio".to_owned(),
                TrackKind::Other(code) => format!("other:{code}"),
            };
            let entry_kind = track
                .entry()
                .map_or_else(|| "none".to_owned(), |entry| entry.kind.to_string());
            let children: Vec<String> = track.entry().map_or_else(Vec::new, |entry| {
                entry
                    .range
                    .children
                    .iter()
                    .map(|child| child.kind.to_string())
                    .collect()
            });
            let compressor = track
                .entry()
                .and_then(|entry| entry.visual.as_ref())
                .map_or(String::new(), |visual| visual.compressor_name.clone());
            let trak: Vec<String> = track
                .range
                .children
                .iter()
                .map(|child| child.kind.to_string())
                .collect();
            let stbl: Vec<String> = track
                .range
                .descend(&[b"mdia", b"minf", b"stbl"])
                .map_or_else(Vec::new, |stbl| {
                    stbl.children.iter().map(|child| child.kind.to_string()).collect()
                });
            observations.push(format!(
                "track {} {kind} {entry_kind} [{}] compressor {compressor:?} handler {:?} flags {:#x} \
                 timescale {} trak [{}] stbl [{}]",
                track.index,
                children.join(" "),
                track.hdlr.name,
                track.tkhd.flags,
                track.mdhd.timescale,
                trak.join(" "),
                stbl.join(" "),
            ));
        }
    }

    let joined = observations.join("\n");
    StructuralFingerprint {
        sha256: Digest::of(joined.as_bytes()),
        observations,
    }
}
