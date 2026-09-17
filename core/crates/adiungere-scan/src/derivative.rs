//! The signs that a file was written by a program other than the recorder.
//!
//! A gallery, a cloud service, a phone's import step or a command-line tool each rewrite a recording and
//! each leave the same marks: the movie box moves after the media, the vendor boxes vanish, an encoder tag
//! appears, a camera disappears, the resolution drops. None of these is a verdict about what the pictures
//! show. They are observations about the container, and the product reports them as such so that a person
//! goes back to the memory card for the file the recorder wrote.

use serde::{Deserialize, Serialize};

use crate::grammar::Grammar;
use crate::probe::{ContainerSummary, Probe};

/// One sign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "sign", rename_all = "snake_case")]
pub enum Signal {
    /// The movie box follows the media data; the reference recorder writes it first.
    MoovAfterMdat,
    /// A tag another program's muxer writes was found.
    ForeignMuxerTag {
        /// The tag.
        tag: String,
    },
    /// The file holds one video track where another file of the same grammar holds two.
    SingleVideoTrackWhereSiblingHasTwo,
    /// The vendor boxes a file of this grammar carries are absent.
    VendorBoxesAbsent,
    /// The resolution is below the largest a sibling of the same grammar has.
    ResolutionBelowSiblings {
        /// This file's width.
        width: u16,
        /// This file's height.
        height: u16,
        /// The largest sibling width.
        sibling_width: u16,
        /// The largest sibling height.
        sibling_height: u16,
    },
}

/// What a file's structure shows about where it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Origin {
    /// Whether the file's name fits a grammar measured on a recorder, so that a layout is known to compare
    /// with. Without one the signs below are limited to what any container shows.
    pub layout_known: bool,
    /// The signs found, in a fixed order; empty when none was.
    pub signals: Vec<Signal>,
}

/// Assesses one probe against the others of the same scan.
#[must_use]
pub fn assess(probe: &Probe, siblings: &[Probe]) -> Origin {
    let layout_known = probe
        .parsed
        .as_ref()
        .is_some_and(|parsed| parsed.grammar.measured());
    let mut signals = Vec::new();

    let Some(summary) = probe.container.as_ref() else {
        return Origin {
            layout_known,
            signals,
        };
    };

    if summary.moov_before_mdat == Some(false) {
        signals.push(Signal::MoovAfterMdat);
    }

    for tag in &summary.foreign_muxer_tags {
        signals.push(Signal::ForeignMuxerTag { tag: tag.clone() });
    }

    // The other files of the population, with what their containers say; a file that could not be read
    // says nothing, and the probe is not its own sibling.
    let comparable: Vec<&ContainerSummary> = siblings
        .iter()
        .filter(|sibling| sibling.name != probe.name)
        .filter(|sibling| are_siblings(probe, sibling))
        .filter_map(|sibling| sibling.container.as_ref())
        .collect();

    let sibling_has_two = comparable.iter().any(|other| other.video_tracks >= 2);
    if summary.video_tracks == 1 && sibling_has_two {
        signals.push(Signal::SingleVideoTrackWhereSiblingHasTwo);
    }

    let grammar_expects_vendor = probe
        .parsed
        .as_ref()
        .is_some_and(|parsed| grammar_carries_vendor_boxes(parsed.grammar));
    let sibling_has_vendor = comparable.iter().any(|other| other.vendor_boxes > 0);
    if summary.vendor_boxes == 0 && (grammar_expects_vendor || sibling_has_vendor) {
        signals.push(Signal::VendorBoxesAbsent);
    }

    let largest = comparable
        .iter()
        .filter_map(|other| Some((other.width?, other.height?)))
        .max();
    if let (Some(width), Some(height), Some((sibling_width, sibling_height))) =
        (summary.width, summary.height, largest)
        && (width < sibling_width || height < sibling_height)
    {
        signals.push(Signal::ResolutionBelowSiblings {
            width,
            height,
            sibling_width,
            sibling_height,
        });
    }

    Origin {
        layout_known,
        signals,
    }
}

/// Whether two probes are files of one population, so that one says what the other should look like:
/// both names fit a grammar, and either the grammar is the same or the names carry the same time. The
/// second case is a file derived from a recorder file and named after it, with a camera word appended.
/// Two names that fit no grammar have nothing in common.
fn are_siblings(probe: &Probe, sibling: &Probe) -> bool {
    match (&probe.parsed, &sibling.parsed) {
        (Some(mine), Some(theirs)) => {
            mine.grammar == theirs.grammar || (mine.time.is_some() && mine.time == theirs.time)
        },
        _ => false,
    }
}

/// Whether a grammar is one whose files carry vendor boxes, which is what the absence sign is measured
/// against when no sibling is available.
const fn grammar_carries_vendor_boxes(grammar: Grammar) -> bool {
    matches!(grammar, Grammar::TimestampEventLetter)
}

#[cfg(test)]
mod tests {
    use super::{Signal, assess};
    use crate::grammar::parse_name;
    use crate::probe::{ContainerSummary, Probe};

    /// A summary of a file the reference recorder could have written: two cameras, the vendor boxes,
    /// the movie box first, no foreign tag.
    fn recorder_summary() -> ContainerSummary {
        ContainerSummary {
            video_tracks: 2,
            audio_tracks: 1,
            enabled_video_tracks: 2,
            moov_before_mdat: Some(true),
            vendor_boxes: 2,
            encoder_tags: vec!["a recorder".to_owned()],
            width: Some(1920),
            height: Some(1080),
            duration_milliseconds: Some(60_000),
            container_time: None,
            foreign_muxer_tags: Vec::new(),
        }
    }

    fn probe(name: &str, summary: Option<ContainerSummary>) -> Probe {
        Probe {
            name: name.to_owned(),
            size: 1,
            parsed: parse_name(name),
            container: summary,
            unreadable: None,
            bytes_read: 0,
        }
    }

    #[test]
    fn a_recorder_file_alone_shows_no_sign_and_a_known_layout() {
        // Act
        let origin = assess(&probe("20260604_122323E.MP4", Some(recorder_summary())), &[]);

        // Assert
        assert!(origin.layout_known);
        assert!(origin.signals.is_empty(), "{:?}", origin.signals);
    }

    #[test]
    fn the_movie_box_after_the_media_and_a_foreign_tag_are_signs_on_any_file() {
        // Arrange
        let summary = ContainerSummary {
            moov_before_mdat: Some(false),
            foreign_muxer_tags: vec!["Lavf".to_owned()],
            ..recorder_summary()
        };

        // Act
        let origin = assess(&probe("holiday.mp4", Some(summary)), &[]);

        // Assert
        assert!(!origin.layout_known);
        assert_eq!(
            origin.signals,
            vec![
                Signal::MoovAfterMdat,
                Signal::ForeignMuxerTag {
                    tag: "Lavf".to_owned()
                }
            ]
        );
    }

    #[test]
    fn a_file_that_could_not_be_read_shows_no_sign() {
        // Act
        let origin = assess(&probe("20260604_122323E.MP4", None), &[]);

        // Assert
        assert!(origin.layout_known);
        assert!(origin.signals.is_empty());
    }

    #[test]
    fn missing_vendor_boxes_are_a_sign_under_the_measured_grammar_and_not_under_another_without_a_sibling() {
        // Arrange
        let bare = ContainerSummary {
            vendor_boxes: 0,
            ..recorder_summary()
        };

        let bare_sibling = probe("20260604_122423_F.mp4", Some(bare.clone()));

        // Act
        let measured = assess(&probe("20260604_122323E.MP4", Some(bare.clone())), &[]);
        let paired = assess(&probe("20260604_122323_F.mp4", Some(bare.clone())), &[]);
        let paired_beside_a_bare_sibling =
            assess(&probe("20260604_122323_F.mp4", Some(bare)), &[bare_sibling]);

        // Assert
        assert_eq!(measured.signals, vec![Signal::VendorBoxesAbsent]);
        assert!(paired.signals.is_empty(), "{:?}", paired.signals);
        assert!(
            paired_beside_a_bare_sibling.signals.is_empty(),
            "a sibling that carries none says nothing: {:?}",
            paired_beside_a_bare_sibling.signals
        );
    }

    #[test]
    fn a_sibling_of_the_same_grammar_says_what_a_file_should_hold() {
        // Arrange
        let recorder = probe("20260101_101010_F.mp4", Some(recorder_summary()));
        let reduced = ContainerSummary {
            video_tracks: 1,
            vendor_boxes: 0,
            width: Some(1280),
            height: Some(720),
            ..recorder_summary()
        };
        let suspect = probe("20260101_101110_F.mp4", Some(reduced));

        // Act
        let origin = assess(&suspect, &[recorder.clone(), suspect.clone()]);
        let recorder_origin = assess(&recorder, &[recorder.clone(), suspect]);

        // Assert
        assert_eq!(
            origin.signals,
            vec![
                Signal::SingleVideoTrackWhereSiblingHasTwo,
                Signal::VendorBoxesAbsent,
                Signal::ResolutionBelowSiblings {
                    width: 1280,
                    height: 720,
                    sibling_width: 1920,
                    sibling_height: 1080,
                },
            ]
        );
        assert!(
            recorder_origin.signals.is_empty(),
            "{:?}",
            recorder_origin.signals
        );
    }

    #[test]
    fn a_file_named_after_a_recorder_file_with_a_camera_word_is_compared_with_it() {
        // A single-camera file written by another program from the recorder file, named after it.

        // Arrange
        let recorder = probe("20260605_081530E.MP4", Some(recorder_summary()));
        let derived = probe(
            "20260605_081530E_rear.MP4",
            Some(ContainerSummary {
                video_tracks: 1,
                vendor_boxes: 0,
                moov_before_mdat: Some(false),
                foreign_muxer_tags: vec!["Lavf".to_owned()],
                ..recorder_summary()
            }),
        );

        // Act
        let origin = assess(&derived, &[recorder, derived.clone()]);

        // Assert
        assert_eq!(
            origin.signals,
            vec![
                Signal::MoovAfterMdat,
                Signal::ForeignMuxerTag {
                    tag: "Lavf".to_owned()
                },
                Signal::SingleVideoTrackWhereSiblingHasTwo,
                Signal::VendorBoxesAbsent,
            ]
        );
    }

    #[test]
    fn a_smaller_width_or_a_smaller_height_alone_is_below_the_siblings_and_an_equal_size_is_not() {
        // Arrange
        let recorder = probe("20260101_101010_F.mp4", Some(recorder_summary()));
        let narrower = probe(
            "20260101_101110_F.mp4",
            Some(ContainerSummary {
                width: Some(1600),
                ..recorder_summary()
            }),
        );
        let shorter = probe(
            "20260101_101210_F.mp4",
            Some(ContainerSummary {
                height: Some(1000),
                ..recorder_summary()
            }),
        );
        let equal = probe("20260101_101310_F.mp4", Some(recorder_summary()));
        let population = [recorder, narrower.clone(), shorter.clone(), equal.clone()];

        // Act and assert
        assert_eq!(
            assess(&narrower, &population).signals,
            vec![Signal::ResolutionBelowSiblings {
                width: 1600,
                height: 1080,
                sibling_width: 1920,
                sibling_height: 1080,
            }]
        );
        assert_eq!(
            assess(&shorter, &population).signals,
            vec![Signal::ResolutionBelowSiblings {
                width: 1920,
                height: 1000,
                sibling_width: 1920,
                sibling_height: 1080,
            }]
        );
        assert!(assess(&equal, &population).signals.is_empty());
    }

    #[test]
    fn two_files_that_fit_no_grammar_are_not_compared_and_neither_are_two_grammars_with_different_times() {
        // Arrange
        let rich = probe("holiday.mp4", Some(recorder_summary()));
        let plain = probe(
            "dinner.mp4",
            Some(ContainerSummary {
                video_tracks: 1,
                vendor_boxes: 0,
                width: Some(640),
                height: Some(480),
                ..recorder_summary()
            }),
        );
        let other_time = probe(
            "20260101_101010_R.mp4",
            Some(ContainerSummary {
                video_tracks: 1,
                vendor_boxes: 0,
                ..recorder_summary()
            }),
        );
        let recorder = probe("20260604_122323E.MP4", Some(recorder_summary()));

        // Act
        let unnamed = assess(&plain, &[rich, plain.clone()]);
        let unrelated = assess(&other_time, &[recorder, other_time.clone()]);

        // Assert
        assert!(unnamed.signals.is_empty(), "{:?}", unnamed.signals);
        assert!(unrelated.signals.is_empty(), "{:?}", unrelated.signals);
    }

    #[test]
    fn a_sibling_that_could_not_be_read_says_nothing() {
        // Arrange
        let unreadable = probe("20260101_101010_F.mp4", None);
        let lone = probe(
            "20260101_101110_F.mp4",
            Some(ContainerSummary {
                video_tracks: 1,
                vendor_boxes: 0,
                ..recorder_summary()
            }),
        );

        // Act
        let origin = assess(&lone, &[unreadable, lone.clone()]);

        // Assert
        assert!(origin.signals.is_empty(), "{:?}", origin.signals);
    }
}
