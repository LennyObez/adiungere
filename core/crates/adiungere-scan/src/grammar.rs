//! Naming grammars: what a recorder's file names say before a byte is read.
//!
//! No brand is named here. A grammar is a shape: a timestamp, a suffix that says which camera, a letter
//! that says why the recording was kept. The reference recorder's grammar was measured on its files; the
//! paired-file grammars describe the common shapes of recorders that write one file per camera, and a name
//! that fits one of them is a *candidate*, confirmed or refuted by reading the file.

use adiungere_manifest::time::Civil;
use serde::{Deserialize, Serialize};

/// Which camera, or which cameras, a file holds according to its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Camera {
    /// One file holding every camera the recorder has.
    All,
    /// The front camera.
    Front,
    /// The rear camera.
    Rear,
    /// An interior camera.
    Interior,
}

/// Why a recording was kept, according to its name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Event {
    /// Continuous recording.
    Normal,
    /// The recorder detected an impact or a hard manoeuvre.
    Event,
    /// A person pressed the button.
    Manual,
    /// The recorder was parked.
    Parking,
}

/// The grammars, each named by its shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grammar {
    /// `YYYYMMDD_HHMMSS` then one letter, `E`, `M` or `N`, then the extension. One file, every camera.
    /// Measured on the reference recorder.
    TimestampEventLetter,
    /// A stem then `F`, `R` or `I` directly before the extension, or `_F`, `_R`, `_I` with an optional
    /// `H` or `L` for the stream quality. One file per camera, paired by the stem.
    StemCameraLetter,
    /// A stem then `-front`, `-back`, `-rear` or `-interior`, with a hyphen or an underscore, before the
    /// extension. One file per camera, paired by the stem.
    StemCameraWord,
}

impl Grammar {
    /// Whether the grammar was measured on a recorder's files rather than described from documentation.
    #[must_use]
    pub const fn measured(self) -> bool {
        matches!(self, Self::TimestampEventLetter)
    }

    /// One sentence describing the shape.
    #[must_use]
    pub const fn describe(self) -> &'static str {
        match self {
            Self::TimestampEventLetter => {
                "a timestamp, one letter for why the recording was kept, one file for every camera"
            },
            Self::StemCameraLetter => "a shared stem and a letter for the camera, one file per camera",
            Self::StemCameraWord => "a shared stem and a word for the camera, one file per camera",
        }
    }
}

/// What a file name says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedName {
    /// The grammar that fitted.
    pub grammar: Grammar,
    /// The part shared by the files of one recording, used to pair them.
    pub stem: String,
    /// Which camera the file holds.
    pub camera: Camera,
    /// Why the recording was kept, when the name says.
    pub event: Option<Event>,
    /// The local time in the name, when the name carries one.
    pub time: Option<Civil>,
}

/// The extensions a recording may carry, compared without regard to case.
const EXTENSIONS: [&str; 2] = ["mp4", "mov"];

/// Splits a file name into its stem and its extension, when the extension is one a recording carries.
fn split_extension(name: &str) -> Option<(&str, &str)> {
    let (stem, extension) = name.rsplit_once('.')?;
    let lowered = extension.to_ascii_lowercase();
    EXTENSIONS
        .contains(&lowered.as_str())
        .then_some((stem, extension))
}

/// Reads exactly `count` decimal digits from the start of a text. A sign or a space is not a digit, even
/// though the standard parser would accept one.
fn digits(text: &str, count: usize) -> Option<u32> {
    let slice = text.get(..count)?;
    if !slice.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    slice.parse().ok()
}

/// Reads `YYYYMMDD_HHMMSS` or `YYYY-MM-DD_HH-MM-SS` from the start of a text, returning the time and
/// how many characters it consumed.
fn timestamp(text: &str) -> Option<(Civil, usize)> {
    let compact = || {
        let year = digits(text, 4)?;
        let month = digits(text.get(4..)?, 2)?;
        let day = digits(text.get(6..)?, 2)?;
        if text.get(8..9)? != "_" {
            return None;
        }
        let hour = digits(text.get(9..)?, 2)?;
        let minute = digits(text.get(11..)?, 2)?;
        let second = digits(text.get(13..)?, 2)?;
        Some((year, month, day, hour, minute, second, 15))
    };
    let dashed = || {
        let year = digits(text, 4)?;
        let month = digits(text.get(5..)?, 2)?;
        let day = digits(text.get(8..)?, 2)?;
        if text.get(4..5)? != "-" || text.get(7..8)? != "-" || text.get(10..11)? != "_" {
            return None;
        }
        let hour = digits(text.get(11..)?, 2)?;
        let minute = digits(text.get(14..)?, 2)?;
        let second = digits(text.get(17..)?, 2)?;
        if text.get(13..14)? != "-" || text.get(16..17)? != "-" {
            return None;
        }
        Some((year, month, day, hour, minute, second, 19))
    };

    let (year, month, day, hour, minute, second, consumed) = compact().or_else(dashed)?;
    let valid = (1..=12).contains(&month)
        && (1..=31).contains(&day)
        && hour < 24
        && minute < 60
        && second < 60
        && (2000..=2099).contains(&year);
    if !valid {
        return None;
    }
    Some((
        Civil {
            year: i64::from(year),
            month: u8::try_from(month).unwrap_or(1),
            day: u8::try_from(day).unwrap_or(1),
            hour: u8::try_from(hour).unwrap_or(0),
            minute: u8::try_from(minute).unwrap_or(0),
            second: u8::try_from(second).unwrap_or(0),
        },
        consumed,
    ))
}

/// Reads a file name under every grammar, first fit wins.
#[must_use]
pub fn parse_name(name: &str) -> Option<ParsedName> {
    let (stem, _) = split_extension(name)?;
    timestamp_event_letter(stem)
        .or_else(|| stem_camera_word(stem))
        .or_else(|| stem_camera_letter(stem))
}

fn timestamp_event_letter(stem: &str) -> Option<ParsedName> {
    let (time, consumed) = timestamp(stem)?;
    let rest = stem.get(consumed..)?;
    let event = match rest {
        "E" => Event::Event,
        "M" => Event::Manual,
        "N" => Event::Normal,
        _ => return None,
    };
    Some(ParsedName {
        grammar: Grammar::TimestampEventLetter,
        stem: stem.get(..consumed)?.to_owned(),
        camera: Camera::All,
        event: Some(event),
        time: Some(time),
    })
}

fn stem_camera_word(stem: &str) -> Option<ParsedName> {
    const WORDS: [(&str, Camera); 8] = [
        ("-front", Camera::Front),
        ("_front", Camera::Front),
        ("-back", Camera::Rear),
        ("_back", Camera::Rear),
        ("-rear", Camera::Rear),
        ("_rear", Camera::Rear),
        ("-interior", Camera::Interior),
        ("_interior", Camera::Interior),
    ];
    let lowered = stem.to_ascii_lowercase();
    let (suffix, camera) = WORDS
        .iter()
        .find(|(suffix, _)| lowered.ends_with(suffix))
        .copied()?;
    let shared = stem.get(..stem.len().checked_sub(suffix.len())?)?;
    if shared.is_empty() {
        return None;
    }
    Some(ParsedName {
        grammar: Grammar::StemCameraWord,
        stem: shared.to_owned(),
        camera,
        event: None,
        time: timestamp(shared).map(|(time, _)| time),
    })
}

fn stem_camera_letter(stem: &str) -> Option<ParsedName> {
    // `<stem>F`, `<stem>_F`, `<stem>_FH`, `<stem>_FL`; likewise `R` and `I`. The stem has to carry a
    // timestamp, because a bare trailing letter is otherwise just the last letter of a word.
    let mut characters = stem.chars().rev();
    let last = characters.next()?;
    let (quality_stripped, last) = if matches!(last, 'H' | 'L' | 'h' | 'l') {
        let previous = characters.next()?;
        if matches!(previous, 'F' | 'R' | 'I' | 'f' | 'r' | 'i') {
            (stem.get(..stem.len().checked_sub(2)?)?, previous)
        } else {
            (stem.get(..stem.len().checked_sub(1)?)?, last)
        }
    } else {
        (stem.get(..stem.len().checked_sub(1)?)?, last)
    };
    let camera = match last.to_ascii_uppercase() {
        'F' => Camera::Front,
        'R' => Camera::Rear,
        'I' => Camera::Interior,
        _ => return None,
    };
    let shared = quality_stripped.strip_suffix('_').unwrap_or(quality_stripped);
    // The shared stem has to be the timestamp and nothing else: a trailing letter after any other text is
    // the last letter of a word, not a camera.
    let (time, consumed) = timestamp(shared)?;
    if consumed != shared.len() {
        return None;
    }
    Some(ParsedName {
        grammar: Grammar::StemCameraLetter,
        stem: shared.to_owned(),
        camera,
        event: None,
        time: Some(time),
    })
}

#[cfg(test)]
mod tests {
    use super::{Camera, Event, Grammar, parse_name};

    #[test]
    fn the_reference_recorder_grammar_reads_the_time_the_event_and_every_camera() {
        // Act
        let parsed = parse_name("20260604_122323E.MP4").unwrap();

        // Assert
        assert_eq!(parsed.grammar, Grammar::TimestampEventLetter);
        assert_eq!(parsed.camera, Camera::All);
        assert_eq!(parsed.event, Some(Event::Event));
        assert_eq!(parsed.time.unwrap().to_string(), "2026-06-04T12:23:23");
        assert_eq!(parsed.stem, "20260604_122323");
        assert!(parsed.grammar.measured());
    }

    #[test]
    fn a_manual_and_a_normal_letter_are_read_and_another_letter_is_not() {
        // Act and assert
        assert_eq!(
            parse_name("20260604_122323M.mp4").unwrap().event,
            Some(Event::Manual)
        );
        assert_eq!(
            parse_name("20260604_122323N.mov").unwrap().event,
            Some(Event::Normal)
        );
        assert!(parse_name("20260604_122323X.mp4").is_none());
    }

    #[test]
    fn a_letter_suffix_pairs_front_and_rear_by_their_stem() {
        // Act
        let front = parse_name("20260604_122323_F.mp4").unwrap();
        let rear = parse_name("20260604_122323_R.mp4").unwrap();
        let high = parse_name("20260604_122323_FH.mp4").unwrap();
        let bare = parse_name("20260604_122323F.mp4").unwrap();

        // Assert
        assert_eq!(front.camera, Camera::Front);
        assert_eq!(rear.camera, Camera::Rear);
        assert_eq!(front.stem, rear.stem);
        assert_eq!(high.camera, Camera::Front);
        assert_eq!(high.stem, front.stem);
        assert_eq!(bare.stem, front.stem);
        assert_eq!(front.grammar, Grammar::StemCameraLetter);
    }

    #[test]
    fn a_word_suffix_pairs_front_and_back_by_their_stem() {
        // Act
        let front = parse_name("2026-06-04_12-23-23-front.mp4").unwrap();
        let back = parse_name("2026-06-04_12-23-23-back.mp4").unwrap();

        // Assert
        assert_eq!(front.camera, Camera::Front);
        assert_eq!(back.camera, Camera::Rear);
        assert_eq!(front.stem, back.stem);
        assert_eq!(front.time.unwrap().to_string(), "2026-06-04T12:23:23");
        assert_eq!(front.grammar, Grammar::StemCameraWord);
    }

    #[test]
    fn a_name_without_a_timestamp_or_with_an_impossible_one_is_not_a_recording() {
        // Act and assert
        assert!(parse_name("holiday.mp4").is_none());
        assert!(parse_name("20261303_122323E.mp4").is_none());
        assert!(parse_name("20260604_252323E.mp4").is_none());
        assert!(parse_name("notes.txt").is_none());
        assert!(parse_name("BUTTERF.mp4").is_none());
        assert!(parse_name("20260604_122323E_extra.mp4").is_none());
    }

    #[test]
    fn every_field_of_a_timestamp_is_bounded_and_the_bounds_are_inclusive() {
        // Act and assert
        for name in [
            "20260604_122323E.mp4",
            "20260101_000000E.mp4",
            "20261231_235959E.mp4",
            "20000101_000000E.mp4",
            "20991231_235959E.mp4",
        ] {
            assert!(parse_name(name).is_some(), "{name}");
        }
        for name in [
            "20260003_122323E.mp4",
            "20261303_122323E.mp4",
            "20260900_122323E.mp4",
            "20260932_122323E.mp4",
            "20260604_242323E.mp4",
            "20260604_126023E.mp4",
            "20260604_122360E.mp4",
            "19991231_235959E.mp4",
            "21000101_000000E.mp4",
            "2026090a_122323E.mp4",
            "2026+604_122323E.mp4",
            "20260604_+22323E.mp4",
            "20260604-122323E.mp4",
            "2026090_122323E.mp4",
        ] {
            assert!(parse_name(name).is_none(), "{name}");
        }
    }

    #[test]
    fn the_dashed_timestamp_needs_every_separator_in_its_place() {
        // Act and assert
        assert!(parse_name("2026-06-04_12-23-23-front.mp4").is_some());
        for name in [
            "2026x09-03_12-23-23-front.mp4",
            "2026-09x03_12-23-23-front.mp4",
            "2026-06-04x12-23-23-front.mp4",
            "2026-06-04_12x23-23-front.mp4",
            "2026-06-04_12-23x23-front.mp4",
        ] {
            let parsed = parse_name(name).unwrap();
            assert_eq!(parsed.grammar, Grammar::StemCameraWord, "{name}");
            assert!(parsed.time.is_none(), "{name}");
        }
    }

    #[test]
    fn the_interior_camera_letter_is_read_and_the_quality_letter_is_stripped_from_every_camera() {
        // Act
        let interior = parse_name("20260604_122323_I.mp4").unwrap();
        let interior_low = parse_name("20260604_122323_IL.mp4").unwrap();
        let rear_high = parse_name("20260604_122323_RH.mp4").unwrap();
        let bare_low = parse_name("20260604_122323RL.mp4").unwrap();

        // Assert
        assert_eq!(interior.camera, Camera::Interior);
        assert_eq!(interior_low.camera, Camera::Interior);
        assert_eq!(rear_high.camera, Camera::Rear);
        assert_eq!(bare_low.camera, Camera::Rear);
        assert_eq!(interior.stem, "20260604_122323");
        assert_eq!(bare_low.stem, "20260604_122323");
        assert!(parse_name("20260604_122323_XH.mp4").is_none());
        assert!(parse_name("20260604_122323_H.mp4").is_none());
    }

    #[test]
    fn only_the_reference_grammar_is_measured_and_each_grammar_describes_itself_in_its_own_words() {
        // Act
        let descriptions: Vec<&str> = [
            Grammar::TimestampEventLetter,
            Grammar::StemCameraLetter,
            Grammar::StemCameraWord,
        ]
        .iter()
        .map(|grammar| grammar.describe())
        .collect();

        // Assert
        assert!(Grammar::TimestampEventLetter.measured());
        assert!(!Grammar::StemCameraLetter.measured());
        assert!(!Grammar::StemCameraWord.measured());
        assert!(
            descriptions.iter().all(|text| text.len() > 20),
            "{descriptions:?}"
        );
        let distinct: std::collections::BTreeSet<&str> = descriptions.iter().copied().collect();
        assert_eq!(distinct.len(), 3, "{descriptions:?}");
    }

    #[test]
    fn a_word_suffix_after_an_underscore_names_a_camera_and_keeps_the_rest_as_the_stem() {
        // A derived file named after a recorder file, with the camera appended: its stem is the recorder
        // file's whole name, so it never pairs with the recorder file, which holds both cameras itself.

        // Act
        let parsed = parse_name("20260604_122323E_rear.MP4").unwrap();

        // Assert
        assert_eq!(parsed.grammar, Grammar::StemCameraWord);
        assert_eq!(parsed.camera, Camera::Rear);
        assert_eq!(parsed.stem, "20260604_122323E");
        assert_eq!(parsed.time.unwrap().to_string(), "2026-06-04T12:23:23");
    }
}
