//! The remuxer against the synthetic corpus: what it writes reads back as the tracks it was given, byte
//! for byte, with the recorder's boxes carried across and the movie box first.

use adiungere_fixtures::{Spec, TOP_LEVEL_BOX, VENDOR_BOX, build, corpus};
use adiungere_isobmff::{
    Container, Error, Input, Placement, Progress, RemuxPlan, RemuxReport, SampleTable, SliceSource, Source,
    SourceError, parse, remux,
};
use sha2::{Digest as _, Sha256};

/// The digests of the three extraction modes of the reference-like recording, pinned once so that the
/// writer is held to the same bytes on every platform it runs on. A change to these numbers is a change
/// to what the product writes, and is reviewed as such.
const FRONT_ONLY: &str = "946dec457228fd5037b9ed7ff670aaf8726fcdccc030f8db5cbfa79c5ea6d03b";
const REAR_ONLY: &str = "b5a99af6122de924a74995658ecff42b0079ec96a73f3aef70dad2cecdd5b07a";
const TWO_TRACK: &str = "ebb5e0fb5b0e4b4d1837753d6fd18b99467d99a2d75d07028a619d30d3c0af64";

fn digest(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// The element at a position, which the test asserts exists.
fn nth<T>(items: &[T], position: usize) -> Option<&T> {
    items.get(position)
}

/// Remuxes the given tracks of one recording into memory.
fn extract(bytes: &[u8], tracks: &[usize]) -> Result<(Vec<u8>, RemuxReport), Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: tracks.to_vec(),
    }];
    let mut out = Vec::new();
    let report = remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true)?;
    Ok((out, report))
}

/// Every sample of every track of a recording, as stored, or the first failure.
fn samples_of(bytes: &[u8]) -> Result<Vec<Vec<Vec<u8>>>, Error> {
    let mut source = SliceSource::new(bytes);
    let container = parse(&mut source)?;
    let mut tracks = Vec::new();
    for track in container.tracks()? {
        let mut samples = Vec::new();
        for sample in track.table.samples()? {
            samples.push(SampleTable::read_sample(&mut source, &sample)?);
        }
        tracks.push(samples);
    }
    Ok(tracks)
}

fn parsed(bytes: &[u8]) -> Result<Container, Error> {
    parse(&mut SliceSource::new(bytes))
}

#[test]
fn a_front_only_extraction_keeps_the_front_samples_the_audio_and_every_vendor_box() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let original = fixture.bytes().unwrap();
    let source_samples = samples_of(&original).unwrap();

    // Act
    let (out, report) = extract(&original, &[0, 2]).unwrap();

    // Assert
    let container = parsed(&out).unwrap();
    assert!(container.ranges_tile_the_file());
    assert_eq!(container.moov_before_mdat(), Some(true));
    assert_eq!(container.length(), out.len() as u64);
    let output_samples = samples_of(&out).unwrap();
    assert_eq!(output_samples.len(), 2);
    assert_eq!(
        nth(&output_samples, 0),
        nth(&source_samples, 0),
        "the front samples"
    );
    assert_eq!(
        nth(&output_samples, 1),
        nth(&source_samples, 2),
        "the audio samples"
    );
    let udta = container.udta_children();
    assert_eq!(udta.len(), 1);
    assert_eq!(
        container.bytes_of(udta.first().unwrap()).unwrap(),
        fixture.expected.vendor_box.as_ref().unwrap().as_slice()
    );
    let top = container.unknown_top_level();
    assert_eq!(top.len(), 1);
    assert_eq!(
        container.bytes_of(top.first().unwrap()).unwrap(),
        fixture.expected.top_level_box.as_ref().unwrap().as_slice()
    );
    let kinds: Vec<String> = container
        .top_level()
        .iter()
        .map(|range| range.kind.to_string())
        .collect();
    assert_eq!(kinds, vec!["ftyp", "moov", "zzzz", "mdat"]);
    assert_eq!(report.bytes_written, out.len() as u64);
    assert!(!report.wide_offsets);
    assert!(!report.renumbered);
    assert_eq!(report.tracks.len(), 2);
    assert_eq!(
        report
            .tracks
            .iter()
            .map(|track| (track.source_index, track.track_id))
            .collect::<Vec<_>>(),
        vec![(0, 1), (2, 3)]
    );
    assert_eq!(
        report
            .preserved
            .iter()
            .map(|b| (b.kind, b.placement))
            .collect::<Vec<_>>(),
        vec![
            (VENDOR_BOX.into(), Placement::UserData),
            (TOP_LEVEL_BOX.into(), Placement::TopLevel)
        ]
    );
}

#[test]
fn the_tracks_keep_their_headers_and_their_tables_except_the_chunk_offsets() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let original = fixture.bytes().unwrap();
    let before = parsed(&original).unwrap();
    let source_tracks = before.tracks().unwrap();

    // Act
    let (out, _) = extract(&original, &[1, 2]).unwrap();

    // Assert
    let after = parsed(&out).unwrap();
    let output_tracks = after.tracks().unwrap();
    for (output, source) in output_tracks
        .iter()
        .zip([nth(&source_tracks, 1).unwrap(), nth(&source_tracks, 2).unwrap()])
    {
        assert_eq!(output.tkhd, source.tkhd);
        assert_eq!(output.mdhd, source.mdhd);
        assert_eq!(output.hdlr, source.hdlr);
        assert_eq!(output.elst, source.elst);
        assert_eq!(output.table.time_to_sample, source.table.time_to_sample);
        assert_eq!(output.table.sample_to_chunk, source.table.sample_to_chunk);
        assert_eq!(output.table.sizes, source.table.sizes);
        assert_eq!(output.table.sync, source.table.sync);
        assert_eq!(output.table.composition, source.table.composition);
        assert_eq!(output.table.chunk_offsets.len(), source.table.chunk_offsets.len());
        assert_ne!(output.table.chunk_offsets, source.table.chunk_offsets);
        let entry = output.entry().unwrap();
        let source_entry = source.entry().unwrap();
        assert_eq!(
            after.bytes_of(&entry.range).unwrap(),
            before.bytes_of(&source_entry.range).unwrap()
        );
    }
}

#[test]
fn the_three_modes_write_the_pinned_bytes() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();

    // Act
    let (front, _) = extract(&original, &[0, 2]).unwrap();
    let (rear, _) = extract(&original, &[1, 2]).unwrap();
    let (both, _) = extract(&original, &[0, 1, 2]).unwrap();

    // Assert
    assert_eq!(
        [digest(&front), digest(&rear), digest(&both)],
        [FRONT_ONLY, REAR_ONLY, TWO_TRACK],
        "front only, rear only, two-track archive"
    );
}

#[test]
fn remuxing_twice_writes_the_same_bytes_and_remuxing_the_output_is_the_identity() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();

    // Act
    let (first, _) = extract(&original, &[0, 1, 2]).unwrap();
    let (second, _) = extract(&original, &[0, 1, 2]).unwrap();
    let (again, _) = extract(&first, &[0, 1, 2]).unwrap();

    // Assert
    assert_eq!(first, second);
    assert_eq!(again, first, "a remux of a remux changes nothing");
}

#[test]
fn every_corpus_recording_remuxes_to_a_recording_that_reads_back_as_itself() {
    let mut checked = 0;
    for spec in corpus() {
        // Arrange
        let fixture = build(&spec).unwrap();
        let Some(original) = fixture.bytes() else {
            continue;
        };
        let source_samples = samples_of(&original).unwrap();
        let all: Vec<usize> = (0..source_samples.len()).collect();

        // Act
        let (out, report) = extract(&original, &all).unwrap();

        // Assert
        let container = parsed(&out).unwrap();
        assert!(container.ranges_tile_the_file(), "{}", spec.name);
        assert_eq!(container.moov_before_mdat(), Some(true), "{}", spec.name);
        assert_eq!(samples_of(&out).unwrap(), source_samples, "{}", spec.name);
        assert_eq!(
            container.udta_children().len(),
            usize::from(spec.vendor.udta_box),
            "{}",
            spec.name
        );
        assert_eq!(
            container.unknown_top_level().len(),
            usize::from(spec.vendor.top_level_box),
            "{}",
            spec.name
        );
        assert_eq!(report.tracks.len(), source_samples.len(), "{}", spec.name);
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} recordings were remuxed");
}

#[test]
fn the_progress_is_reported_after_every_chunk_and_can_stop_the_remux() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut source = SliceSource::new(&original);
    let container = parse(&mut source).unwrap();
    let mut seen: Vec<Progress> = Vec::new();

    // Act
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![0, 1, 2],
    }];
    let mut out = Vec::new();
    let report = remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |progress| {
        seen.push(*progress);
        true
    })
    .unwrap();

    let mut stopped = Vec::new();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![0, 1, 2],
    }];
    let cancelled = remux(&mut inputs, &RemuxPlan::default(), &mut stopped, &mut |_| false);

    // Assert
    assert_eq!(seen.len(), usize::try_from(report.chunks).unwrap());
    assert!(seen.windows(2).all(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(a, b)| a.bytes_written < b.bytes_written)
    }));
    assert!(
        seen.iter()
            .all(|progress| progress.bytes_total == report.bytes_written)
    );
    assert_eq!(seen.last().unwrap().bytes_written, report.bytes_written);
    assert!(
        matches!(cancelled, Err(Error::Cancelled { bytes_written }) if bytes_written == stopped.len() as u64),
        "{cancelled:?}"
    );
    assert!(stopped.len() < out.len());
}

#[test]
fn a_missing_track_a_track_selected_twice_and_no_track_at_all_are_refused() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();

    // Act and assert
    assert!(matches!(
        extract(&original, &[3]),
        Err(Error::NoSuchTrack { input: 0, index: 3 })
    ));
    assert!(matches!(
        extract(&original, &[0, 0]),
        Err(Error::NoSuchTrack { input: 0, index: 0 })
    ));
    assert!(matches!(extract(&original, &[]), Err(Error::NothingSelected)));
}

#[test]
fn a_plan_that_drops_the_vendor_boxes_writes_none() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut source = SliceSource::new(&original);
    let container = parse(&mut source).unwrap();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![0],
    }];
    let mut out = Vec::new();

    // Act
    let report = remux(
        &mut inputs,
        &RemuxPlan {
            keep_udta: false,
            keep_unknown_top_level: false,
        },
        &mut out,
        &mut |_| true,
    )
    .unwrap();

    // Assert
    let container = parsed(&out).unwrap();
    assert!(container.udta_children().is_empty());
    assert!(container.unknown_top_level().is_empty());
    assert!(report.preserved.is_empty());
    assert!(container.moov().child(b"udta").is_none());
}

#[test]
fn two_recordings_join_into_one_with_renumbered_tracks() {
    // A recorder that writes one file per camera: the front file's video and audio, the rear file's video.

    // Arrange
    let front_file = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let rear_file = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let front_samples = samples_of(&front_file).unwrap();
    let mut front_source = SliceSource::new(&front_file);
    let mut rear_source = SliceSource::new(&rear_file);
    let front = parse(&mut front_source).unwrap();
    let rear = parse(&mut rear_source).unwrap();
    let mut inputs = [
        Input {
            container: &front,
            source: &mut front_source,
            tracks: vec![0, 2],
        },
        Input {
            container: &rear,
            source: &mut rear_source,
            tracks: vec![1],
        },
    ];
    let mut out = Vec::new();

    // Act
    let report = remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true).unwrap();

    // Assert
    assert!(report.renumbered);
    assert_eq!(
        report
            .tracks
            .iter()
            .map(|t| (t.input, t.source_index, t.track_id, t.source_track_id))
            .collect::<Vec<_>>(),
        vec![(0, 0, 1, 1), (0, 2, 2, 3), (1, 1, 3, 2)]
    );
    let container = parsed(&out).unwrap();
    let tracks = container.tracks().unwrap();
    assert_eq!(
        tracks.iter().map(|track| track.tkhd.track_id).collect::<Vec<_>>(),
        vec![1, 2, 3]
    );
    assert_eq!(container.mvhd().unwrap().next_track_id, 4);
    let output_samples = samples_of(&out).unwrap();
    assert_eq!(nth(&output_samples, 0), nth(&front_samples, 0));
    assert_eq!(nth(&output_samples, 1), nth(&front_samples, 2));
    assert_eq!(nth(&output_samples, 2), nth(&front_samples, 1));
    assert!(container.ranges_tile_the_file());
}

/// A source that serves a recording's bytes and claims that every sample is a run of zeros of enormous
/// size, for exercising the offsets past four gibibytes without a file that large.
struct Stretched {
    header: Vec<u8>,
    length: u64,
}

impl Source for Stretched {
    fn length(&mut self) -> Result<u64, SourceError> {
        Ok(self.length)
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        into.fill(0);
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        if let Some(head) = self.header.get(start..) {
            let overlap = head.len().min(into.len());
            if let (Some(to), Some(from)) = (into.get_mut(..overlap), head.get(..overlap)) {
                to.copy_from_slice(from);
            }
        }
        Ok(())
    }
}

/// A sink that keeps the first bytes and counts the rest.
struct Head {
    kept: Vec<u8>,
    total: u64,
}

impl std::io::Write for Head {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let room = (1 << 20) - self.kept.len().min(1 << 20);
        self.kept
            .extend_from_slice(buffer.get(..buffer.len().min(room)).unwrap_or(buffer));
        self.total += buffer.len() as u64;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn media_past_four_gibibytes_gets_wide_offsets_and_a_large_media_data_box() {
    // Arrange: every sample of every track declared as sixty-four mebibytes, in a source that serves
    // zeros there; a hundred and five of them end well past four gibibytes.
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut patched = original.clone();
    let size_tables: Vec<u64> = {
        let container = parsed(&original).unwrap();
        container
            .tracks()
            .unwrap()
            .iter()
            .map(|track| {
                track
                    .range
                    .descend(&[b"mdia", b"minf", b"stbl", b"stsz"])
                    .unwrap()
                    .payload_offset()
            })
            .collect()
    };
    for table in size_tables {
        let at = usize::try_from(table).unwrap() + 4;
        patched
            .get_mut(at..at + 4)
            .unwrap()
            .copy_from_slice(&(64u32 << 20).to_be_bytes());
    }
    let mut source = Stretched {
        header: patched,
        length: 1 << 40,
    };
    let container = parse(&mut source).unwrap();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![0, 1, 2],
    }];
    let mut sink = Head {
        kept: Vec::new(),
        total: 0,
    };

    // Act
    let report = remux(&mut inputs, &RemuxPlan::default(), &mut sink, &mut |_| true).unwrap();

    // Assert
    assert!(report.wide_offsets);
    assert_eq!(report.bytes_written, sink.total);
    assert!(sink.total > 105 * (64 << 20));
    let head = parse(&mut Stretched {
        header: sink.kept,
        length: sink.total,
    })
    .unwrap();
    let mdat = head.mdat();
    assert_eq!(mdat.len(), 1);
    assert_eq!(
        mdat.first().unwrap().header,
        16,
        "the media data box uses the long size"
    );
    let tracks = head.tracks().unwrap();
    assert!(tracks.iter().all(|track| track.table.wide_offsets));
    assert!(
        tracks
            .iter()
            .flat_map(|track| track.table.chunk_offsets.iter())
            .any(|offset| *offset > u64::from(u32::MAX)),
        "the last chunks lie past four gibibytes"
    );
    assert_eq!(
        nth(&tracks, 0).unwrap().table.chunk_offsets.len(),
        2,
        "the front track keeps its two chunks"
    );
}
