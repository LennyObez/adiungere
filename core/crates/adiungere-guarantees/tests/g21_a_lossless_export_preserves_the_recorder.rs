//! **G21.** Every lossless export carries the recorder's boxes byte for byte, writes the movie box before
//! the media data, and gives every kept track the fingerprint it had in the source.
//!
//! The reason the product exists is that every other route drops the recorder's boxes. So each export mode
//! of each corpus recording is written and read back, and three things are compared with the source: the
//! bytes of every user-data child and every unknown top-level box, the position of the movie box, and the
//! track fingerprint and elementary stream digest of every kept track. A detection test shows that the same
//! comparison catches an output whose vendor box lost one byte.

use adiungere_fingerprint::{annex_b_digest, track_fingerprint};
use adiungere_fixtures::{Spec, build, corpus};
use adiungere_isobmff::{Container, Input, RemuxPlan, SliceSource, TrackKind, parse, remux};

/// The bytes of every user-data child and every unknown top-level box, by type, in file order.
fn preserved_boxes(container: &Container) -> Vec<(String, Vec<u8>)> {
    let mut boxes = Vec::new();
    for range in container.udta_children() {
        boxes.push((
            range.kind.to_string(),
            container.bytes_of(range).unwrap_or(&[]).to_vec(),
        ));
    }
    for range in container.unknown_top_level() {
        boxes.push((
            range.kind.to_string(),
            container.bytes_of(range).unwrap_or(&[]).to_vec(),
        ));
    }
    boxes
}

/// The fingerprints of the given tracks, and the stream digests of the video ones among them.
fn fingerprints(
    bytes: &[u8],
    tracks: &[usize],
) -> Result<Vec<(String, Option<String>)>, adiungere_fingerprint::Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let all = container.tracks()?;
    let mut digests = Vec::new();
    for track in tracks.iter().filter_map(|&index| all.get(index)) {
        let fingerprint = track_fingerprint(&mut source, &container, track)?;
        let stream = if track.kind == TrackKind::Video {
            Some(annex_b_digest(&mut source, &container, track)?.sha256.to_string())
        } else {
            None
        };
        digests.push((fingerprint.payload_sha256.to_string(), stream));
    }
    Ok(digests)
}

fn export(bytes: &[u8], tracks: &[usize]) -> Result<Vec<u8>, adiungere_isobmff::Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: tracks.to_vec(),
    }];
    let mut out = Vec::new();
    remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true)?;
    Ok(out)
}

#[test]
fn every_export_mode_of_every_corpus_recording_preserves_the_recorder_and_the_fingerprints() {
    let mut checked = 0usize;
    for spec in corpus() {
        // Arrange
        let Some(original) = build(&spec).unwrap().bytes() else {
            continue;
        };
        let source = parse(&mut SliceSource::new(&original)).unwrap();
        let track_count = source.tracks().unwrap().len();
        let videos: Vec<usize> = source
            .tracks()
            .unwrap()
            .iter()
            .filter(|track| track.kind == TrackKind::Video)
            .map(|track| track.index)
            .collect();
        let audios: Vec<usize> = (0..track_count).filter(|index| !videos.contains(index)).collect();
        let mut modes: Vec<Vec<usize>> = vec![(0..track_count).collect()];
        for video in &videos {
            let mut tracks = vec![*video];
            tracks.extend(audios.iter().copied());
            tracks.sort_unstable();
            modes.push(tracks);
        }

        for tracks in modes {
            // Act
            let out = export(&original, &tracks).unwrap();

            // Assert
            let written = parse(&mut SliceSource::new(&out)).unwrap();
            assert_eq!(written.moov_before_mdat(), Some(true), "{} {tracks:?}", spec.name);
            assert_eq!(
                preserved_boxes(&written),
                preserved_boxes(&source),
                "{} {tracks:?}: the recorder's boxes",
                spec.name
            );
            let kept: Vec<usize> = (0..tracks.len()).collect();
            assert_eq!(
                fingerprints(&out, &kept).unwrap(),
                fingerprints(&original, &tracks).unwrap(),
                "{} {tracks:?}: the fingerprints",
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
fn the_comparison_would_notice_one_byte_lost_from_a_vendor_box() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let source = parse(&mut SliceSource::new(&original)).unwrap();
    let mut out = export(&original, &[0, 2]).unwrap();
    let written = parse(&mut SliceSource::new(&out)).unwrap();
    let vendor = written.udta_children().first().copied().unwrap().clone();
    let position = usize::try_from(vendor.offset).unwrap() + 100;

    // Act
    if let Some(byte) = out.get_mut(position) {
        *byte ^= 0x01;
    }
    let damaged = parse(&mut SliceSource::new(&out)).unwrap();

    // Assert
    assert_ne!(preserved_boxes(&damaged), preserved_boxes(&source));
}
