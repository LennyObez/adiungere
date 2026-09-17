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
            usize::from(spec.vendor.udta_box) + usize::from(spec.vendor.encoder_tag_box),
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

impl Head {
    fn new() -> Self {
        Self {
            kept: Vec::new(),
            total: 0,
        }
    }
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

/// The reference-like recording with every sample of every track declared at one size, which the sample
/// size tables' constant-size field states in one place per track.
fn declared_at(sample_size: u32) -> Result<Vec<u8>, Error> {
    let original = build(&Spec::reference_like())
        .map_err(|_| Error::NothingSelected)?
        .bytes()
        .ok_or(Error::NothingSelected)?;
    let mut patched = original.clone();
    let container = parsed(&original)?;
    for track in container.tracks()? {
        let table = track
            .range
            .descend(&[b"mdia", b"minf", b"stbl", b"stsz"])
            .ok_or(Error::NothingSelected)?
            .payload_offset();
        let at = usize::try_from(table).map_err(|_| Error::NothingSelected)? + 4;
        patched
            .get_mut(at..at + 4)
            .ok_or(Error::NothingSelected)?
            .copy_from_slice(&sample_size.to_be_bytes());
    }
    Ok(patched)
}

#[test]
fn media_past_four_gibibytes_gets_wide_offsets_and_a_large_media_data_box() {
    // Arrange: every sample of every track declared as sixty-four mebibytes, in a source that serves
    // zeros there; a hundred and five of them end well past four gibibytes.
    let mut source = Stretched {
        header: declared_at(64 << 20).unwrap(),
        length: 1 << 40,
    };
    let container = parse(&mut source).unwrap();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![0, 1, 2],
    }];
    let mut sink = Head::new();

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

/// A source that counts how many times it is asked for bytes.
struct Counted<S> {
    inner: S,
    requests: u32,
}

impl<S: Source> Source for Counted<S> {
    fn length(&mut self) -> Result<u64, SourceError> {
        self.inner.length()
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        self.requests += 1;
        self.inner.read_at(offset, into)
    }
}

/// The most media the remuxer reads in one request: a chunk stored in one run and no larger than this is
/// one request, anything else is one request per sample.
const COPY_BUFFER: u64 = 16 << 20;

/// What a remux of every track asked its source, against what the chunk sizes say it should have asked:
/// the requests made, the requests expected, the largest chunk in bytes and the smallest.
struct Requests {
    made: u32,
    expected: u32,
    chunks: u32,
    largest_chunk: u64,
    smallest_chunk: u64,
}

fn requests_of<S: Source>(source: S) -> Result<Requests, Error> {
    let mut counted = Counted {
        inner: source,
        requests: 0,
    };
    let container = parse(&mut counted)?;
    let mut chunks: Vec<(u64, u32)> = Vec::new();
    for track in container.tracks()? {
        let mut current: Option<(u32, u64, u32)> = None;
        for sample in track.table.samples()? {
            match current.as_mut() {
                Some((chunk, bytes, count)) if *chunk == sample.chunk => {
                    *bytes += u64::from(sample.size);
                    *count += 1;
                },
                _ => {
                    if let Some((_, bytes, count)) = current.take() {
                        chunks.push((bytes, count));
                    }
                    current = Some((sample.chunk, u64::from(sample.size), 1));
                },
            }
        }
        if let Some((_, bytes, count)) = current.take() {
            chunks.push((bytes, count));
        }
    }
    let expected = chunks
        .iter()
        .map(|(bytes, count)| if *bytes <= COPY_BUFFER { 1 } else { *count })
        .sum();
    counted.requests = 0;
    let mut inputs = [Input {
        container: &container,
        source: &mut counted,
        tracks: vec![0, 1, 2],
    }];
    remux(&mut inputs, &RemuxPlan::default(), &mut Head::new(), &mut |_| {
        true
    })?;
    Ok(Requests {
        made: counted.requests,
        expected,
        chunks: u32::try_from(chunks.len()).unwrap_or(u32::MAX),
        largest_chunk: chunks.iter().map(|(bytes, _)| *bytes).max().unwrap_or(0),
        smallest_chunk: chunks.iter().map(|(bytes, _)| *bytes).min().unwrap_or(0),
    })
}

#[test]
fn a_chunk_stored_in_one_run_is_read_in_one_request_up_to_the_copy_buffer() {
    // The recorder stores the samples of a chunk back to back, so the copy asks the source once per
    // chunk and not once per sample, up to the sixteen mebibytes the copy buffer holds; a larger chunk is
    // read sample by sample, so no chunk a hostile file declares decides how much memory is allocated.
    // Three recordings: the small one, one whose chunks lie between one and sixteen mebibytes, and one
    // whose chunks all lie past the buffer.

    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let within_the_buffer = Stretched {
        header: declared_at(256 << 10).unwrap(),
        length: 1 << 30,
    };
    let past_the_buffer = Stretched {
        header: declared_at(8 << 20).unwrap(),
        length: 1 << 34,
    };

    // Act
    let small = requests_of(SliceSource::new(&original)).unwrap();
    let mid = requests_of(within_the_buffer).unwrap();
    let large = requests_of(past_the_buffer).unwrap();

    // Assert
    assert!(small.largest_chunk < 1 << 20, "{}", small.largest_chunk);
    assert_eq!(small.made, small.expected, "one request per small chunk");
    assert!(
        mid.largest_chunk > 1 << 20 && mid.largest_chunk <= COPY_BUFFER,
        "{} to {}",
        mid.smallest_chunk,
        mid.largest_chunk
    );
    assert_eq!(mid.made, mid.expected, "one request per chunk within the buffer");
    assert!(large.largest_chunk > COPY_BUFFER, "{}", large.largest_chunk);
    assert!(
        large.expected > large.chunks,
        "at least one chunk is read sample by sample"
    );
    assert_eq!(
        large.made, large.expected,
        "one request per sample past the buffer"
    );
}

/// A sink that accepts every byte and refuses to flush.
struct Unflushable;

impl std::io::Write for Unflushable {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Err(std::io::Error::other("the device is gone"))
    }
}

#[test]
fn a_sink_that_cannot_flush_fails_the_remux_rather_than_reporting_it_written() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut source = SliceSource::new(&original);
    let container = parse(&mut source).unwrap();
    let mut inputs = [Input {
        container: &container,
        source: &mut source,
        tracks: vec![0, 2],
    }];

    // Act
    let outcome = remux(&mut inputs, &RemuxPlan::default(), &mut Unflushable, &mut |_| {
        true
    });

    // Assert
    assert!(
        matches!(outcome, Err(Error::Write(ref cause)) if cause.to_string() == "the device is gone"),
        "{outcome:?}"
    );
}

#[test]
fn an_input_prints_its_tracks_and_not_its_source() {
    // Arrange
    let original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut source = SliceSource::new(&original);
    let container = parse(&mut source).unwrap();
    let input = Input {
        container: &container,
        source: &mut source,
        tracks: vec![0, 2],
    };

    // Act
    let printed = format!("{input:?}");

    // Assert
    assert_eq!(printed, "Input { tracks: [0, 2], .. }");
}

#[test]
fn a_join_of_one_track_from_each_file_writes_the_next_identifier_after_its_two_tracks() {
    // The sources both say the next identifier is four; the output has two tracks and says three.

    // Arrange
    let first_file = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let second_file = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let mut first_source = SliceSource::new(&first_file);
    let mut second_source = SliceSource::new(&second_file);
    let first = parse(&mut first_source).unwrap();
    let second = parse(&mut second_source).unwrap();
    assert_eq!(first.mvhd().unwrap().next_track_id, 4);
    let mut inputs = [
        Input {
            container: &first,
            source: &mut first_source,
            tracks: vec![0],
        },
        Input {
            container: &second,
            source: &mut second_source,
            tracks: vec![1],
        },
    ];
    let mut out = Vec::new();

    // Act
    remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true).unwrap();

    // Assert
    let container = parsed(&out).unwrap();
    assert_eq!(container.mvhd().unwrap().next_track_id, 3);
    assert_eq!(
        container
            .tracks()
            .unwrap()
            .iter()
            .map(|track| track.tkhd.track_id)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

/// One track's references: each reference type with the identifiers it names, or nothing for a track
/// without a reference box.
type References = Option<Vec<(String, Vec<u32>)>>;

/// The track references of every track of a recording, in track order.
fn references_of(bytes: &[u8]) -> Result<Vec<References>, Error> {
    let container = parsed(bytes)?;
    let mut all = Vec::new();
    for track in container.tracks()? {
        let Some(tref) = track.range.descend(&[b"tref"]) else {
            all.push(None);
            continue;
        };
        let mut references = Vec::new();
        for reference in &tref.children {
            let payload = container
                .bytes_of(reference)
                .and_then(|bytes| bytes.get(usize::from(reference.header)..))
                .unwrap_or(&[]);
            let ids = payload
                .as_chunks::<4>()
                .0
                .iter()
                .map(|id| u32::from_be_bytes(*id))
                .collect();
            references.push((reference.kind.to_string(), ids));
        }
        all.push(Some(references));
    }
    Ok(all)
}

#[test]
fn track_references_follow_the_tracks_they_name_and_leave_with_them() {
    // The rear track of this recording depends on the front track and describes the audio track.

    // Arrange
    let spec = corpus()
        .into_iter()
        .find(|spec| spec.quirks.track_references)
        .unwrap();
    let original = build(&spec).unwrap().bytes().unwrap();
    assert_eq!(
        references_of(&original).unwrap(),
        vec![
            None,
            Some(vec![("vdep".to_owned(), vec![1]), ("cdsc".to_owned(), vec![3])]),
            None
        ]
    );

    // Act
    let (whole, whole_report) = extract(&original, &[0, 1, 2]).unwrap();
    let (rear_and_audio, rear_report) = extract(&original, &[1, 2]).unwrap();
    let (rear_alone, alone_report) = extract(&original, &[1]).unwrap();

    // Assert
    assert_eq!(references_of(&whole).unwrap(), references_of(&original).unwrap());
    assert_eq!(whole_report.references_dropped, 0);
    assert_eq!(
        references_of(&rear_and_audio).unwrap(),
        vec![Some(vec![("cdsc".to_owned(), vec![3])]), None],
        "the reference to the dropped front track leaves with it"
    );
    assert_eq!(rear_report.references_dropped, 1);
    assert_eq!(
        references_of(&rear_alone).unwrap(),
        vec![None],
        "a reference box left with nothing to name is dropped"
    );
    assert_eq!(alone_report.references_dropped, 2);
    assert!(parsed(&rear_alone).unwrap().ranges_tile_the_file());
}

#[test]
fn a_join_rewrites_the_references_to_the_identifiers_the_output_gives() {
    // Arrange
    let spec = corpus()
        .into_iter()
        .find(|spec| spec.quirks.track_references)
        .unwrap();
    let first_file = build(&spec).unwrap().bytes().unwrap();
    let second_file = build(&spec).unwrap().bytes().unwrap();
    let mut first_source = SliceSource::new(&first_file);
    let mut second_source = SliceSource::new(&second_file);
    let first = parse(&mut first_source).unwrap();
    let second = parse(&mut second_source).unwrap();
    let mut inputs = [
        Input {
            container: &first,
            source: &mut first_source,
            tracks: vec![0],
        },
        Input {
            container: &second,
            source: &mut second_source,
            tracks: vec![2, 1],
        },
    ];
    let mut out = Vec::new();

    // Act
    let report = remux(&mut inputs, &RemuxPlan::default(), &mut out, &mut |_| true).unwrap();

    // Assert: the second file's audio became track 2 and its rear track 3; the rear's reference to its
    // own file's front track, which is not in the output, is gone, and its reference to the audio
    // names that track's new identifier.
    assert_eq!(
        report
            .tracks
            .iter()
            .map(|t| (t.input, t.source_track_id, t.track_id))
            .collect::<Vec<_>>(),
        vec![(0, 1, 1), (1, 3, 2), (1, 2, 3)]
    );
    assert_eq!(
        references_of(&out).unwrap(),
        vec![None, None, Some(vec![("cdsc".to_owned(), vec![2])])]
    );
    assert_eq!(report.references_dropped, 1);
}

#[test]
fn an_unknown_top_level_box_too_large_to_hold_is_still_carried_across() {
    // The reader keeps the bytes of an unknown box only up to a cap; a larger one is a range the writer
    // has to copy from the source, in pieces, rather than refuse.

    // Arrange
    let mut original = build(&Spec::reference_like()).unwrap().bytes().unwrap();
    let payload: Vec<u8> = (0..(3u32 << 19)).map(|i| (i % 251) as u8).collect();
    let mut big = Vec::new();
    big.extend_from_slice(&u32::try_from(payload.len() + 8).unwrap().to_be_bytes());
    big.extend_from_slice(b"abcd");
    big.extend_from_slice(&payload);
    original.extend_from_slice(&big);
    let before = parsed(&original).unwrap();
    let held = before
        .unknown_top_level()
        .iter()
        .map(|range| (range.kind.to_string(), before.bytes_of(range).is_some()))
        .collect::<Vec<_>>();

    // Act
    let (out, report) = extract(&original, &[0, 1, 2]).unwrap();

    // Assert
    assert_eq!(
        held,
        vec![("zzzz".to_owned(), true), ("abcd".to_owned(), false)],
        "the large box is a range without bytes in the reader"
    );
    let container = parsed(&out).unwrap();
    let kinds: Vec<String> = container
        .top_level()
        .iter()
        .map(|range| range.kind.to_string())
        .collect();
    assert_eq!(kinds, vec!["ftyp", "moov", "zzzz", "abcd", "mdat"]);
    let carried = container
        .unknown_top_level()
        .into_iter()
        .find(|range| range.kind == b"abcd")
        .unwrap();
    let start = usize::try_from(carried.offset).unwrap();
    assert_eq!(out.get(start..start + big.len()), Some(big.as_slice()));
    assert!(
        report
            .preserved
            .iter()
            .any(|b| b.kind == b"abcd" && b.size == big.len() as u64)
    );
    assert!(container.ranges_tile_the_file());
}
