//! **G12.** Reading a recording and re-emitting its ranges reproduces the input byte for byte, unknown
//! boxes included, an unknown child inside a sample entry included.
//!
//! The reader keeps every box as a range and never as a value it might resynthesise. The proof is that the
//! ranges tile the file: no gap, no overlap, in order, down to the children of a sample entry. Concatenating
//! the bytes of the top-level ranges, read back through the same source, is then the file. A reader that
//! dropped an unknown box, or absorbed a vendor child into a typed parent, breaks the tiling on the corpus
//! entry that carries the box.

use adiungere_fixtures::{Spec, build, corpus};
use adiungere_isobmff::{Source, parse};

#[test]
fn every_corpus_recording_tiles_and_re_emits_as_itself() {
    // Arrange
    let mut checked = 0usize;

    for spec in corpus() {
        let mut fixture = build(&spec).unwrap();
        let Some(original) = fixture.bytes() else {
            // The sparse recording is checked for tiling alone; its hole is not worth reading back.
            let container = parse(&mut fixture).unwrap();
            assert!(container.ranges_tile_the_file(), "{}", spec.name);
            continue;
        };

        // Act
        let container = parse(&mut fixture).unwrap();
        let mut emitted = Vec::with_capacity(original.len());
        for range in container.top_level() {
            let bytes = fixture
                .read_range(range.offset, usize::try_from(range.size).unwrap())
                .unwrap();
            emitted.extend_from_slice(&bytes);
        }

        // Assert
        assert!(
            container.ranges_tile_the_file(),
            "{}: the ranges do not tile",
            spec.name
        );
        assert_eq!(
            emitted, original,
            "{}: re-emitting the ranges changed a byte",
            spec.name
        );
        checked += 1;
    }

    assert!(
        checked >= 8,
        "only {checked} recordings were round-tripped; this check is inert"
    );
}

#[test]
fn the_unknown_child_inside_a_sample_entry_is_part_of_the_tiling() {
    // The one place a typed reader loses bytes silently: a vendor box after the decoder configuration
    // inside a sample entry, which a typed entry would not have a field for.

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
    assert!(entry.range.children.iter().any(|child| child.kind == b"zunk"));
    assert!(entry.range.children_tile_payload());
}

#[test]
fn the_check_would_notice_a_range_that_stops_short() {
    // A detection test: a container whose ranges do not tile is refused by the same predicate.

    // Arrange
    let mut fixture = build(&Spec::reference_like()).unwrap();
    let container = parse(&mut fixture).unwrap();
    let mut moov = container.moov().clone();
    moov.children.pop();

    // Act
    let tiles = moov.children_tile_payload();

    // Assert
    assert!(!tiles);
}
