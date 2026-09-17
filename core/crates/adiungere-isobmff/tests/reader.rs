//! The reader against the synthetic corpus: what it finds is what was written, and what it reads is
//! bounded.

use std::future::Future;
use std::pin::pin;
use std::task::{Context, Poll, Waker};

use adiungere_fixtures::{ENTRY_CHILD_BOX, Role, Spec, TOP_LEVEL_BOX, VENDOR_BOX, build, corpus};
use adiungere_isobmff::{
    Container, CountingSource, Error, FileSource, FourCc, SampleTable, SliceSource, Source, SourceError,
    TrackKind, parse, parse_async,
};

/// Drives a future that never actually waits, which is what every source in this suite is.
fn block_on<F: Future>(future: F) -> F::Output {
    let mut future = pin!(future);
    let mut context = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(output) = future.as_mut().poll(&mut context) {
            return output;
        }
    }
}

/// Every sample of one track, read through the source, or the first failure.
fn read_all(container: &Container, source: &mut impl Source, index: usize) -> Result<Vec<Vec<u8>>, Error> {
    let tracks = container.tracks()?;
    let track = tracks.get(index).ok_or(Error::NoMovieBox)?;
    track
        .table
        .samples()?
        .iter()
        .map(|sample| SampleTable::read_sample(source, sample))
        .collect()
}

/// A source that claims to be enormous while holding a few bytes, for testing the caps.
struct Huge(Vec<u8>, u64);

impl Source for Huge {
    fn length(&mut self) -> Result<u64, SourceError> {
        Ok(self.1)
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        let start = usize::try_from(offset).unwrap_or(usize::MAX);
        for (slot, index) in into.iter_mut().zip(start..) {
            *slot = self.0.get(index).copied().unwrap_or(0);
        }
        Ok(())
    }
}

#[test]
fn every_corpus_recording_reads_back_exactly_what_was_written() {
    for spec in corpus() {
        // Arrange
        let mut fixture = build(&spec).unwrap();

        // Act
        let container = parse(&mut fixture).unwrap_or_else(|error| panic!("{}: {error}", spec.name));
        let tracks = container.tracks().unwrap();

        // Assert
        assert_eq!(tracks.len(), fixture.expected.tracks.len(), "{}", spec.name);
        for (index, expected) in fixture.expected.tracks.clone().iter().enumerate() {
            let track = tracks.get(index).unwrap();
            assert_eq!(track.tkhd.track_id, expected.track_id, "{}", spec.name);
            let samples = read_all(&container, &mut fixture, index).unwrap();
            assert_eq!(samples, expected.samples, "{} track {index}", spec.name);
            let sync: Vec<u32> = track
                .table
                .samples()
                .unwrap()
                .iter()
                .filter(|sample| sample.sync)
                .map(|sample| sample.index + 1)
                .collect();
            assert_eq!(sync, expected.sync_samples, "{} track {index}", spec.name);
            let entry = track.entry().unwrap();
            let configuration = entry.configuration.as_ref().unwrap();
            assert_eq!(
                container.payload_of(configuration).unwrap(),
                expected.configuration_payload.as_slice(),
                "{} track {index}",
                spec.name
            );
            let kind = match expected.role {
                Role::Front | Role::Rear => TrackKind::Video,
                Role::Audio => TrackKind::Audio,
            };
            assert_eq!(track.kind, kind);
        }
    }
}

#[test]
fn the_vendor_boxes_survive_as_ranges_with_their_bytes() {
    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();
    let udta = container.udta_children();
    let top = container.unknown_top_level();

    // Assert
    assert_eq!(udta.len(), 1);
    let vendor = udta.first().unwrap();
    assert_eq!(vendor.kind, VENDOR_BOX);
    assert_eq!(
        container.bytes_of(vendor).unwrap(),
        fixture.expected.vendor_box.as_ref().unwrap().as_slice()
    );
    assert_eq!(top.len(), 1);
    let model = top.first().unwrap();
    assert_eq!(model.kind, TOP_LEVEL_BOX);
    assert_eq!(model.size, 12);
    assert_eq!(
        container.bytes_of(model).unwrap(),
        fixture.expected.top_level_box.as_ref().unwrap().as_slice()
    );
}

#[test]
fn an_unknown_child_inside_a_sample_entry_is_a_range_and_the_entry_still_tiles() {
    // Arrange
    let spec = corpus()
        .into_iter()
        .find(|spec| spec.quirks.unknown_child_in_entry)
        .unwrap();
    let mut fixture = build(&spec).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();
    let tracks = container.tracks().unwrap();
    let entry = tracks.first().unwrap().entry().unwrap();

    // Assert
    let kinds: Vec<String> = entry
        .range
        .children
        .iter()
        .map(|child| child.kind.to_string())
        .collect();
    assert_eq!(kinds, vec!["avcC", "zunk"]);
    let unknown = entry.range.child(&ENTRY_CHILD_BOX).unwrap();
    assert_eq!(container.payload_of(unknown).unwrap(), b"opaque entry child");
    assert!(entry.range.children_tile_payload());
    assert!(container.ranges_tile_the_file());
}

#[test]
fn the_ranges_tile_every_recording_of_the_corpus() {
    for spec in corpus() {
        // Arrange
        let mut fixture = build(&spec).unwrap();

        // Act
        let container = parse(&mut fixture).unwrap();

        // Assert
        assert!(container.ranges_tile_the_file(), "{}", spec.name);
        assert_eq!(container.length(), fixture.length(), "{}", spec.name);
    }
}

#[test]
fn reading_the_structure_never_touches_the_media_and_costs_less_than_the_budget() {
    for spec in corpus() {
        // Arrange
        let fixture = build(&spec).unwrap();
        let mdat_start = fixture.expected.mdat_offset;
        let mdat_end = mdat_start + fixture.expected.mdat_size;
        let mut counting = CountingSource::new(fixture);

        // Act
        parse(&mut counting).unwrap();

        // Assert
        // The header of the media data box is read; its payload never is.
        assert!(
            !counting.touched(mdat_start + 16, mdat_end),
            "{}: the media payload was read",
            spec.name
        );
        assert!(
            counting.bytes_read() < 256 * 1024,
            "{}: {} bytes were read",
            spec.name,
            counting.bytes_read()
        );
    }
}

#[test]
fn the_asynchronous_driver_issues_the_same_reads_as_the_synchronous_one() {
    for spec in corpus() {
        // Arrange
        let fixture = build(&spec).unwrap();
        let mut synchronous = CountingSource::new(fixture.clone());
        let mut asynchronous = CountingSource::new(fixture);

        // Act
        let blocking = parse(&mut synchronous).unwrap();
        let awaited = block_on(parse_async(&mut asynchronous)).unwrap();

        // Assert
        assert_eq!(synchronous.reads(), asynchronous.reads(), "{}", spec.name);
        assert_eq!(blocking.top_level(), awaited.top_level(), "{}", spec.name);
    }
}

#[test]
fn the_movie_box_position_is_reported() {
    // Arrange
    let mut first = build(&Spec::reference_like()).unwrap();
    let last_spec = corpus()
        .into_iter()
        .find(|spec| spec.name == "moov-last")
        .unwrap();
    let mut last = build(&last_spec).unwrap();

    // Act
    let first_container = parse(&mut first).unwrap();
    let last_container = parse(&mut last).unwrap();

    // Assert
    assert_eq!(first_container.moov_before_mdat(), Some(true));
    assert_eq!(last_container.moov_before_mdat(), Some(false));
}

#[test]
fn chunk_offsets_beyond_four_gigabytes_are_followed() {
    // Arrange
    let spec = corpus().into_iter().find(|spec| spec.layout.sparse).unwrap();
    let mut fixture = build(&spec).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();
    let tracks = container.tracks().unwrap();
    let first = tracks.first().unwrap();
    let samples = first.table.samples().unwrap();

    // Assert
    assert!(first.table.wide_offsets);
    assert!(samples.first().unwrap().offset > 4 << 30);
    let bytes = read_all(&container, &mut fixture, 0).unwrap();
    assert_eq!(bytes, fixture.expected.tracks.first().unwrap().samples);
}

#[test]
fn an_edit_list_is_read_and_left_as_data() {
    // Arrange
    let spec = corpus().into_iter().find(|spec| spec.quirks.edit_list).unwrap();
    let mut fixture = build(&spec).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();
    let tracks = container.tracks().unwrap();

    // Assert
    let elst = tracks.first().unwrap().elst.as_ref().unwrap();
    assert_eq!(elst.entries.len(), 1);
    assert_eq!(elst.entries.first().unwrap().1, 1000);
}

#[test]
fn a_file_that_is_not_a_base_media_file_is_refused() {
    // Arrange
    let bytes = b"not a movie at all, just some text that is long enough";
    let mut source = SliceSource::new(bytes);

    // Act
    let outcome = parse(&mut source);

    // Assert
    assert!(matches!(outcome, Err(Error::NotIsobmff)), "got {outcome:?}");
}

#[test]
fn a_file_without_a_movie_box_is_refused() {
    // Arrange
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&16u32.to_be_bytes());
    bytes.extend_from_slice(b"ftypmp42");
    bytes.extend_from_slice(&[0, 0, 0, 0]);
    bytes.extend_from_slice(&8u32.to_be_bytes());
    bytes.extend_from_slice(b"mdat");
    let mut source = SliceSource::new(&bytes);

    // Act
    let outcome = parse(&mut source);

    // Assert
    assert!(matches!(outcome, Err(Error::NoMovieBox)), "got {outcome:?}");
}

#[test]
fn a_truncated_movie_box_is_refused_with_its_offset() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let moov_offset = usize::try_from(fixture.expected.moov_offset).unwrap();
    let truncated = bytes.get(..moov_offset + 100).unwrap();
    let mut source = SliceSource::new(truncated);

    // Act
    let outcome = parse(&mut source);

    // Assert
    assert!(
        matches!(outcome, Err(Error::Overrun { offset, .. }) if offset == fixture.expected.moov_offset),
        "got {outcome:?}"
    );
}

#[test]
fn a_second_movie_box_is_refused_rather_than_chosen_between() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let moov_start = usize::try_from(fixture.expected.moov_offset).unwrap();
    let moov_size = u32::from_be_bytes(bytes.get(moov_start..moov_start + 4).unwrap().try_into().unwrap());
    let moov: Vec<u8> = bytes
        .get(moov_start..moov_start + usize::try_from(moov_size).unwrap())
        .unwrap()
        .to_vec();
    let second_at = bytes.len() as u64;
    bytes.extend_from_slice(&moov);
    let mut source = SliceSource::new(&bytes);

    // Act
    let outcome = parse(&mut source);

    // Assert
    assert!(
        matches!(outcome, Err(Error::TwoMovieBoxes { offset }) if offset == second_at),
        "got {outcome:?}"
    );
}

#[test]
fn a_movie_box_larger_than_the_cap_is_refused_before_it_is_read() {
    // Arrange
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u32.to_be_bytes());
    bytes.extend_from_slice(b"moov");
    bytes.extend_from_slice(&(1u64 << 40).to_be_bytes());
    let mut source = Huge(bytes, 1u64 << 40);

    // Act
    let outcome = parse(&mut source);

    // Assert
    assert!(matches!(outcome, Err(Error::Oversized { .. })), "got {outcome:?}");
}

#[test]
fn the_sample_entries_expose_their_visual_and_audio_fields() {
    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();
    let tracks = container.tracks().unwrap();
    let video = tracks.first().unwrap().entry().unwrap();
    let audio = tracks.get(2).unwrap().entry().unwrap();

    // Assert
    let visual = video.visual.as_ref().unwrap();
    assert_eq!((visual.width, visual.height), (64, 64));
    assert_eq!(visual.frame_count, 1);
    assert_eq!(visual.compressor_name, "synthetic pattern");
    assert_eq!(visual.depth, 0x18);
    assert!(video.audio.is_none());
    assert_eq!(video.data_reference_index, 1);
    assert_eq!(video.codec_string(&container).as_deref(), Some("avc1.64000A"));
    let sound = audio.audio.as_ref().unwrap();
    assert_eq!(sound.version, 0);
    assert_eq!(sound.channel_count, 2);
    assert_eq!(sound.sample_size, 16);
    assert_eq!(sound.sample_rate, 44100);
    assert!(audio.visual.is_none());
    assert!(audio.codec_string(&container).is_none());
    assert!(audio.avc_configuration(&container).is_none());
}

#[test]
fn the_tracks_expose_their_headers_durations_and_totals() {
    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();
    let tracks = container.tracks().unwrap();
    let video = tracks.first().unwrap();
    let audio = tracks.get(2).unwrap();

    // Assert
    assert!(video.tkhd.is_enabled());
    assert_eq!(video.tkhd.flags, 1);
    assert_eq!(video.mdhd.language, "und");
    assert_eq!(video.mdhd.timescale, 30000);
    assert_eq!(video.duration_milliseconds(), Some(1000));
    assert_eq!(audio.duration_milliseconds(), Some(1044));
    assert_eq!(video.hdlr.name, "VideoHandler");
    assert_eq!(video.table.sync_sample_count(), 2);
    assert_eq!(audio.table.sync_sample_count(), 45);
    assert!(!video.table.has_composition_offsets());
    let expected_bytes: u64 = fixture.expected.tracks[0]
        .samples
        .iter()
        .map(|sample| sample.len() as u64)
        .sum();
    assert_eq!(video.table.total_sample_bytes(), expected_bytes);
    assert!(video.elst.is_none());
    let mvhd = container.mvhd().unwrap();
    assert_eq!(mvhd.timescale, 60000);
    assert_eq!(mvhd.next_track_id, 4);
    assert_eq!(mvhd.creation_time, adiungere_fixtures::CREATION_TIME);
}

#[test]
fn the_container_accessors_agree_with_the_layout() {
    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();

    // Act
    let container = parse(&mut fixture).unwrap();

    // Assert
    let ftyp = container.ftyp().unwrap();
    assert_eq!(ftyp.major_brand, b"mp42");
    assert_eq!(ftyp.compatible_brands, vec![FourCc(*b"mp42"), FourCc(*b"mp41")]);
    let mdat = container.mdat();
    assert_eq!(mdat.len(), 1);
    assert_eq!(mdat.first().unwrap().offset, fixture.expected.mdat_offset);
    assert_eq!(mdat.first().unwrap().size, fixture.expected.mdat_size);
    let paths: Vec<String> = container.walk().into_iter().map(|(path, _)| path).collect();
    assert!(paths.contains(&"moov/udta/zvnd".to_owned()));
    assert!(paths.contains(&"moov/trak/mdia/minf/stbl/stsd/avc1/avcC".to_owned()));
    assert_eq!(paths.first().map(String::as_str), Some("ftyp"));
    assert_eq!(container.top_level().len(), 4);
    assert_eq!(container.moov().kind, b"moov");
    assert!(
        container.bytes_of(mdat.first().unwrap()).is_none(),
        "the media is never held"
    );
}

#[test]
fn a_file_source_reads_exactly_what_a_slice_source_reads() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let directory = std::env::temp_dir().join(format!("adiungere-isobmff-{}", std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    let path = directory.join("clip.mp4");
    std::fs::write(&path, &bytes).unwrap();
    let mut file = FileSource::open(&path).unwrap();

    // Act
    let length = file.length().unwrap();
    let head = file.read_range(0, 8).unwrap();
    let tail = file.read_range(length - 4, 4).unwrap();
    let past = file.read_range(length - 4, 5);
    let overflow = file.read_range(u64::MAX, 1);
    let container = parse(&mut file).unwrap();
    let _ = std::fs::remove_dir_all(&directory);

    // Assert
    assert_eq!(length, bytes.len() as u64);
    assert_eq!(head, bytes[..8]);
    assert_eq!(tail, bytes[bytes.len() - 4..]);
    assert!(matches!(past, Err(SourceError::OutOfBounds { .. })));
    assert!(matches!(overflow, Err(SourceError::OutOfBounds { .. })));
    assert_eq!(container.top_level().len(), 4);
    assert!(FileSource::open(&directory.join("missing.mp4")).is_err());
}

#[test]
fn a_file_that_opens_with_a_sixty_four_bit_size_is_read() {
    // Arrange: the same recording with the file type box rewritten to a 16-byte header.
    let fixture = build(&Spec::reference_like()).unwrap();
    let bytes = fixture.bytes().unwrap();
    let ftyp_size = u32::from_be_bytes(bytes[..4].try_into().unwrap());
    let mut rewritten = Vec::new();
    rewritten.extend_from_slice(&1u32.to_be_bytes());
    rewritten.extend_from_slice(b"ftyp");
    rewritten.extend_from_slice(&(u64::from(ftyp_size) + 8).to_be_bytes());
    rewritten.extend_from_slice(&bytes[8..usize::try_from(ftyp_size).unwrap()]);
    let shift = 8u64;
    // The chunk offsets in the movie box no longer point at the media; only the structure is read here.
    rewritten.extend_from_slice(&bytes[usize::try_from(ftyp_size).unwrap()..]);
    let mut source = SliceSource::new(&rewritten);

    // Act
    let container = parse(&mut source).unwrap();

    // Assert
    let ftyp = container.top_level().first().unwrap();
    assert_eq!(ftyp.header, 16);
    assert_eq!(ftyp.size, u64::from(ftyp_size) + shift);
    assert_eq!(container.ftyp().unwrap().major_brand, b"mp42");
    assert_eq!(container.moov().offset, u64::from(ftyp_size) + shift);
    assert!(container.ranges_tile_the_file());
}

#[test]
fn bytes_are_held_for_a_range_inside_a_held_box_and_for_nothing_that_crosses_its_edge() {
    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();
    let container = parse(&mut fixture).unwrap();
    let moov = container.moov().clone();
    let mut before = moov.clone();
    before.offset -= 4;
    let mut after = moov.clone();
    after.size += 4;
    let mut inside = moov.clone();
    inside.offset += 8;
    inside.size -= 8;

    // Act and assert
    assert!(container.bytes_of(&moov).is_some());
    assert!(container.bytes_of(&inside).is_some());
    assert!(container.bytes_of(&before).is_none());
    assert!(container.bytes_of(&after).is_none());
}

#[test]
fn a_source_error_carries_its_operating_system_cause() {
    // Arrange
    let io = SourceError::Io(std::io::Error::other("closed"));
    let bounds = SourceError::OutOfBounds {
        offset: 0,
        length: 1,
        available: 0,
    };

    // Act and assert
    assert!(std::error::Error::source(&io).is_some());
    assert!(std::error::Error::source(&bounds).is_none());
    assert_eq!(io.to_string(), "closed");
}

#[test]
fn a_recording_whose_last_box_is_a_bare_header_ends_in_a_box_and_not_a_tail() {
    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    bytes.extend_from_slice(&[0, 0, 0, 8]);
    bytes.extend_from_slice(b"free");
    let mut source = SliceSource::new(&bytes);

    // Act
    let container = parse(&mut source).unwrap();

    // Assert
    let last = container.top_level().last().unwrap();
    assert_eq!(last.kind, b"free");
    assert_eq!(last.size, 8);
    assert!(!last.clipped);
    assert!(container.ranges_tile_the_file());
}

#[test]
fn bytes_after_the_last_box_are_a_clipped_tail_once_the_movie_box_was_read() {
    // A recorder that pre-allocates its file leaves unused space after the last box it wrote.

    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    bytes.extend_from_slice(&[0; 4096]);
    let mut source = SliceSource::new(&bytes);

    // Act
    let container = parse(&mut source).unwrap();

    // Assert
    let tail = container.clipped_tail().unwrap();
    assert_eq!(tail.size, 4096);
    assert_eq!(tail.offset, fixture.length());
    assert_eq!(tail.header, 0);
    assert!(container.ranges_tile_the_file());
    let unknown: Vec<FourCc> = container
        .unknown_top_level()
        .iter()
        .map(|range| range.kind)
        .collect();
    assert_eq!(unknown, vec![TOP_LEVEL_BOX], "a tail is not an unknown box");
    assert_eq!(container.tracks().unwrap().len(), 3);
}

#[test]
fn a_media_data_box_that_declares_more_than_the_file_holds_is_clipped_once_the_movie_box_was_read() {
    // A recorder that lost power leaves a media data box whose header promised the whole recording.

    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut bytes = fixture.bytes().unwrap();
    let mdat = usize::try_from(fixture.expected.mdat_offset).unwrap();
    let declared = u32::try_from(fixture.expected.mdat_size + 100_000).unwrap();
    bytes[mdat..mdat + 4].copy_from_slice(&declared.to_be_bytes());
    let mut source = SliceSource::new(&bytes);

    // Act
    let container = parse(&mut source).unwrap();

    // Assert
    let tail = container.clipped_tail().unwrap();
    assert_eq!(tail.kind, b"mdat");
    assert_eq!(tail.size, fixture.expected.mdat_size);
    assert_eq!(tail.header, 8);
    assert!(container.ranges_tile_the_file());
    assert_eq!(container.moov_before_mdat(), Some(true));
}

#[test]
fn a_header_that_does_not_read_is_a_tail_after_the_movie_box_and_an_error_before_it() {
    // A size of two declares a box smaller than its own header.

    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let mut after = fixture.bytes().unwrap();
    after.extend_from_slice(&[0, 0, 0, 2, b'j', b'u', b'n', b'k', 9, 9]);
    let spec = corpus()
        .into_iter()
        .find(|spec| spec.name == "moov-last")
        .unwrap();
    let last = build(&spec).unwrap().bytes().unwrap();
    let ftyp_size = usize::try_from(u32::from_be_bytes([last[0], last[1], last[2], last[3]])).unwrap();
    let mut before = last[..ftyp_size].to_vec();
    before.extend_from_slice(&[0, 0, 0, 2, b'j', b'u', b'n', b'k']);
    before.extend_from_slice(&last[ftyp_size..]);

    // Act
    let tolerated = parse(&mut SliceSource::new(&after)).unwrap();
    let refused = parse(&mut SliceSource::new(&before));

    // Assert
    let tail = tolerated.clipped_tail().unwrap();
    assert_eq!(tail.size, 10);
    assert_eq!(tail.header, 0);
    assert!(tolerated.ranges_tile_the_file());
    assert!(
        matches!(refused, Err(Error::Undersized { size: 2, .. })),
        "got {refused:?}"
    );
}

#[test]
fn bytes_that_are_not_a_box_before_the_movie_box_are_still_refused() {
    // Without the movie box there is nothing to describe, so a tail before it is not tolerated.

    // Arrange
    let spec = corpus()
        .into_iter()
        .find(|spec| spec.name == "moov-last")
        .unwrap();
    let fixture = build(&spec).unwrap();
    let bytes = fixture.bytes().unwrap();
    let ftyp_size = usize::try_from(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])).unwrap();
    assert_eq!(&bytes[4..8], b"ftyp");
    let mut damaged = bytes[..ftyp_size].to_vec();
    damaged.extend_from_slice(b"\xff\xff\xff\xffjunk");
    damaged.extend_from_slice(&bytes[ftyp_size..]);
    let mut source = SliceSource::new(&damaged);

    // Act
    let outcome = parse(&mut source);

    // Assert
    assert!(matches!(outcome, Err(Error::Overrun { .. })), "got {outcome:?}");
}

#[test]
fn the_error_messages_name_the_box_and_the_offset() {
    // Arrange
    let error = Error::Overrun {
        kind: FourCc(*b"moov"),
        offset: 20,
        size: 500,
        limit: 100,
    };
    let source_error = SourceError::OutOfBounds {
        offset: 7,
        length: 9,
        available: 8,
    };

    // Act
    let text = error.to_string();
    let source_text = source_error.to_string();

    // Assert
    assert_eq!(
        text,
        "the moov box at 20 declares 500 bytes and would end past 100"
    );
    assert_eq!(source_text, "9 bytes at 7 were asked for and the source holds 8");
    assert!(
        std::error::Error::source(&Error::Source(SourceError::OutOfBounds {
            offset: 0,
            length: 0,
            available: 0
        }))
        .is_some()
    );
    assert!(std::error::Error::source(&Error::NoMovieBox).is_none());
    assert!(
        std::error::Error::source(&Error::Write(std::io::Error::other("full")))
            .is_some_and(|cause| cause.to_string() == "full")
    );
}

#[test]
fn mutated_recordings_never_panic_the_reader() {
    // A deterministic mutation pass, run on every pull request. The nightly fuzzer goes much further; this
    // is the smoke that keeps a panic from reaching the default branch between two nights.

    // Arrange
    let fixture = build(&Spec::reference_like()).unwrap();
    let original = fixture.bytes().unwrap();
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut accepted = 0usize;
    let mut refused = 0usize;
    let length = original.len() as u64;

    for _ in 0..2000 {
        let mut bytes = original.clone();
        for _ in 0..4 {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let position = usize::try_from(state % length).unwrap();
            let value = u8::try_from((state >> 32) & 0xff).unwrap();
            if let Some(slot) = bytes.get_mut(position) {
                *slot = value;
            }
        }
        let cut = if state & 0x10 == 0 {
            bytes.len()
        } else {
            usize::try_from((state >> 8) % length).unwrap()
        };
        bytes.truncate(cut.max(1));

        // Act
        let mut source = SliceSource::new(&bytes);
        match parse(&mut source) {
            Ok(container) => {
                if let Ok(tracks) = container.tracks() {
                    for track in &tracks {
                        let _ = track.table.samples();
                        let _ = track
                            .entry()
                            .and_then(|entry| entry.avc_configuration(&container));
                    }
                }
                let _ = container.mvhd();
                let _ = container.ftyp();
                let _ = container.walk();
                accepted += 1;
            },
            Err(_) => refused += 1,
        }
    }

    // Assert
    assert_eq!(accepted + refused, 2000);
    assert!(
        refused > 0,
        "no mutation was refused, so the mutations did nothing"
    );
}
