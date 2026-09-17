//! **G19.** Inspecting a recording's structure reads fewer than 256 kibibytes and never touches the media
//! data box.
//!
//! A library holds thousands of recordings of a hundred and forty megabytes each. A scan that read each
//! one would take an afternoon and a battery; a scan that read the movie box alone takes seconds. The
//! bound is measured through a counting source rather than assumed from the code, on every corpus
//! recording, with the movie box first and with the movie box last.

use adiungere_fixtures::corpus;
use adiungere_isobmff::CountingSource;
use adiungere_manifest::{Producer, Scope};

/// The most bytes a structural inspection may read. The reference recording's movie box is about eighty
/// kilobytes; the bound leaves room for a longer recording and none for reading the media.
const BUDGET: u64 = 256 * 1024;

#[test]
fn every_corpus_inspection_stays_under_the_budget_and_off_the_media() {
    // Arrange
    let mut checked = 0usize;

    for spec in corpus() {
        let fixture = adiungere_fixtures::build(&spec).unwrap();
        let mdat_payload_start = fixture.expected.mdat_offset + 16;
        let mdat_end = fixture.expected.mdat_offset + fixture.expected.mdat_size;
        let mut counting = CountingSource::new(fixture);

        // Act
        let manifest = adiungere_manifest::build(
            &mut counting,
            "clip.mp4",
            Scope::Structure,
            &Producer::current("test", "0"),
            None,
        )
        .unwrap();

        // Assert
        assert!(
            counting.bytes_read() < BUDGET,
            "{}: {} bytes were read",
            spec.name,
            counting.bytes_read()
        );
        assert!(
            !counting.touched(mdat_payload_start, mdat_end),
            "{}: the media payload was read",
            spec.name
        );
        assert!(
            manifest.file.sha256.is_none(),
            "{}: a structural inspection digested the file",
            spec.name
        );
        assert!(manifest.tracks.iter().all(|track| track.fingerprint.is_none()));
        checked += 1;
    }

    assert!(
        checked >= 8,
        "only {checked} recordings were inspected; this check is inert"
    );
}

#[test]
fn a_full_fingerprint_does_read_the_media_so_the_counter_is_not_inert() {
    // The counting source has to be able to see a media read, or the guarantee above proves nothing.

    // Arrange
    let fixture = adiungere_fixtures::build(&adiungere_fixtures::Spec::reference_like()).unwrap();
    let mdat_payload_start = fixture.expected.mdat_offset + 16;
    let mut counting = CountingSource::new(fixture);

    // Act
    adiungere_manifest::build(
        &mut counting,
        "clip.mp4",
        Scope::Full,
        &Producer::current("test", "0"),
        None,
    )
    .unwrap();

    // Assert
    assert!(counting.touched(mdat_payload_start, u64::MAX));
}
