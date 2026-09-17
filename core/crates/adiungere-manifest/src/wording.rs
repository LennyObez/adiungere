//! The wording catalogue: every sentence the product forms about a recording, in data.
//!
//! A sentence that reads as a verdict is the one thing this product never says, and the easiest thing to
//! say by accident in a status line. So the sentences live in a catalogue per language, each string is
//! checked against a list of words that read as verdicts, and code renders a [`Phrase`] rather than
//! writing prose. The catalogue is also the single source for translation.

use std::collections::BTreeMap;
use std::sync::LazyLock;

use serde::Deserialize;

/// The English catalogue, as committed.
pub const ENGLISH: &str = include_str!("../wording/en.json");
/// The English list of words no string may contain, as committed.
pub const ENGLISH_FORBIDDEN: &str = include_str!("../wording/en.forbidden.json");

/// A catalogue file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Catalogue {
    /// The language tag.
    pub language: String,
    /// Every string by key.
    pub strings: BTreeMap<String, String>,
}

/// A forbidden-words file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Forbidden {
    /// The language tag.
    pub language: String,
    /// Why the list exists.
    pub why: String,
    /// The words, each matched as a whole word without regard to case.
    pub words: Vec<String>,
}

/// Every sentence the product can form, named. The catalogue must hold exactly these keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Phrase {
    /// The standing disclaimer.
    FactsOnly,
    /// The plainest form of the disclaimer.
    NotAnExamination,
    /// The movie box precedes the media.
    MoovFirst,
    /// The movie box follows the media.
    MoovLast,
    /// There is no media data box.
    MoovNone,
    /// The file ends with bytes that are not a well-formed box.
    ClippedTail,
    /// The track listing.
    Tracks,
    /// Vendor boxes were found.
    VendorPresent,
    /// No vendor box was found.
    VendorAbsent,
    /// User-data boxes of a type a standard defines were found.
    StandardUserData,
    /// Several video tracks are the default.
    DefaultTracks,
    /// Whole file identical.
    FileIdentical,
    /// Whole file differs.
    FileDiffers,
    /// Whole file not recorded.
    FileNotRecorded,
    /// Track identical.
    TrackIdentical,
    /// Track differs.
    TrackDiffers,
    /// Track fingerprint not recorded.
    TrackNotRecorded,
    /// Track could not be compared.
    TrackNotCheckable,
    /// Track recorded and missing from the file.
    TrackMissing,
    /// Vendor box identical.
    VendorIdentical,
    /// Vendor box differs.
    VendorDiffers,
    /// Vendor box recorded and absent.
    VendorMissing,
    /// Vendor box present and unrecorded.
    VendorUnexpected,
    /// Structure matches.
    StructureIdentical,
    /// Structure differs.
    StructureDiffers,
    /// No sign of rewriting.
    RewriteNone,
    /// Signs of rewriting follow.
    RewriteSome,
    /// Sign: movie box after media.
    SignMoovAfterMdat,
    /// Sign: encoder tag of another program.
    SignEncoderTag,
    /// Sign: one video track where a sibling has two.
    SignSingleVideo,
    /// Sign: vendor boxes absent.
    SignVendorAbsent,
    /// Sign: resolution capped.
    SignResolutionCapped,
    /// The naming grammar is unknown.
    UnknownLayout,
    /// A time sentence.
    NoLaterThan,
    /// What the container clock is.
    ContainerClockNote,
    /// What the file name clock is.
    FileNameNote,
    /// What the satellite clock is.
    SatelliteClockNote,
    /// What the system clock is.
    SystemClockNote,
    /// The clocks disagree.
    TimeDisagreement,
    /// A track fingerprint line.
    FingerprintTrack,
    /// An elementary stream digest line.
    FingerprintStream,
    /// A whole-file digest line.
    FingerprintFile,
    /// A structural fingerprint line.
    FingerprintStructure,
    /// A single file with two cameras.
    ScanSingleDual,
    /// A single file with one camera.
    ScanSingleMono,
    /// A matched pair.
    ScanPaired,
    /// An unmatched half of a pair.
    ScanUnpaired,
    /// Not recognised.
    ScanNotRecognised,
    /// Not readable.
    ScanNotReadable,
    /// Files skipped by their extension.
    ScanSkipped,
    /// An export was written.
    ExportWritten,
    /// One track of an export and where it came from.
    ExportTrack,
    /// The boxes an export carried across as bytes.
    ExportPreserved,
    /// An export carried no box across.
    ExportPreservedNone,
    /// Where the export's manifest was written.
    ExportManifest,
    /// What an export shows of people and places, said after every export.
    ExportFaces,
}

impl Phrase {
    /// Every phrase, so that the catalogue can be checked against the code in both directions.
    pub const ALL: [Self; 56] = [
        Self::FactsOnly,
        Self::NotAnExamination,
        Self::MoovFirst,
        Self::MoovLast,
        Self::MoovNone,
        Self::ClippedTail,
        Self::Tracks,
        Self::VendorPresent,
        Self::VendorAbsent,
        Self::StandardUserData,
        Self::DefaultTracks,
        Self::FileIdentical,
        Self::FileDiffers,
        Self::FileNotRecorded,
        Self::TrackIdentical,
        Self::TrackDiffers,
        Self::TrackNotRecorded,
        Self::TrackNotCheckable,
        Self::TrackMissing,
        Self::VendorIdentical,
        Self::VendorDiffers,
        Self::VendorMissing,
        Self::VendorUnexpected,
        Self::StructureIdentical,
        Self::StructureDiffers,
        Self::RewriteNone,
        Self::RewriteSome,
        Self::SignMoovAfterMdat,
        Self::SignEncoderTag,
        Self::SignSingleVideo,
        Self::SignVendorAbsent,
        Self::SignResolutionCapped,
        Self::UnknownLayout,
        Self::NoLaterThan,
        Self::ContainerClockNote,
        Self::FileNameNote,
        Self::SatelliteClockNote,
        Self::SystemClockNote,
        Self::TimeDisagreement,
        Self::FingerprintTrack,
        Self::FingerprintStream,
        Self::FingerprintFile,
        Self::FingerprintStructure,
        Self::ScanSingleDual,
        Self::ScanSingleMono,
        Self::ScanPaired,
        Self::ScanUnpaired,
        Self::ScanNotRecognised,
        Self::ScanNotReadable,
        Self::ScanSkipped,
        Self::ExportWritten,
        Self::ExportTrack,
        Self::ExportPreserved,
        Self::ExportPreservedNone,
        Self::ExportManifest,
        Self::ExportFaces,
    ];

    /// The catalogue key.
    #[must_use]
    pub const fn key(self) -> &'static str {
        match self {
            Self::FactsOnly => "facts.only",
            Self::NotAnExamination => "not.an.examination",
            Self::MoovFirst => "inspection.moov.first",
            Self::MoovLast => "inspection.moov.last",
            Self::MoovNone => "inspection.moov.none",
            Self::ClippedTail => "inspection.tail",
            Self::Tracks => "inspection.tracks",
            Self::VendorPresent => "inspection.vendor.present",
            Self::VendorAbsent => "inspection.vendor.absent",
            Self::StandardUserData => "inspection.user.data.standard",
            Self::DefaultTracks => "inspection.default.tracks",
            Self::FileIdentical => "integrity.file.identical",
            Self::FileDiffers => "integrity.file.differs",
            Self::FileNotRecorded => "integrity.file.not.recorded",
            Self::TrackIdentical => "integrity.track.identical",
            Self::TrackDiffers => "integrity.track.differs",
            Self::TrackNotRecorded => "integrity.track.not.recorded",
            Self::TrackNotCheckable => "integrity.track.not.checkable",
            Self::TrackMissing => "integrity.track.missing",
            Self::VendorIdentical => "integrity.vendor.identical",
            Self::VendorDiffers => "integrity.vendor.differs",
            Self::VendorMissing => "integrity.vendor.absent",
            Self::VendorUnexpected => "integrity.vendor.unexpected",
            Self::StructureIdentical => "integrity.structure.identical",
            Self::StructureDiffers => "integrity.structure.differs",
            Self::RewriteNone => "structure.rewrite.none",
            Self::RewriteSome => "structure.rewrite.some",
            Self::SignMoovAfterMdat => "structure.sign.moov.after.mdat",
            Self::SignEncoderTag => "structure.sign.encoder.tag",
            Self::SignSingleVideo => "structure.sign.single.video",
            Self::SignVendorAbsent => "structure.sign.vendor.absent",
            Self::SignResolutionCapped => "structure.sign.resolution.capped",
            Self::UnknownLayout => "structure.unknown.layout",
            Self::NoLaterThan => "time.no.later.than",
            Self::ContainerClockNote => "time.container.clock.note",
            Self::FileNameNote => "time.file.name.note",
            Self::SatelliteClockNote => "time.satellite.clock.note",
            Self::SystemClockNote => "time.system.clock.note",
            Self::TimeDisagreement => "time.disagreement",
            Self::FingerprintTrack => "fingerprint.track",
            Self::FingerprintStream => "fingerprint.stream",
            Self::FingerprintFile => "fingerprint.file",
            Self::FingerprintStructure => "fingerprint.structure",
            Self::ScanSingleDual => "scan.single.dual",
            Self::ScanSingleMono => "scan.single.mono",
            Self::ScanPaired => "scan.paired",
            Self::ScanUnpaired => "scan.unpaired",
            Self::ScanNotRecognised => "scan.not.recognised",
            Self::ScanNotReadable => "scan.not.readable",
            Self::ScanSkipped => "scan.skipped",
            Self::ExportWritten => "export.written",
            Self::ExportTrack => "export.track",
            Self::ExportPreserved => "export.preserved",
            Self::ExportPreservedNone => "export.preserved.none",
            Self::ExportManifest => "export.manifest",
            Self::ExportFaces => "export.faces",
        }
    }
}

static ENGLISH_CATALOGUE: LazyLock<Result<Catalogue, String>> =
    LazyLock::new(|| serde_json::from_str(ENGLISH).map_err(|error| error.to_string()));

/// Reads a catalogue from its text.
///
/// # Errors
///
/// Returns the parse error as text.
pub fn parse_catalogue(text: &str) -> Result<Catalogue, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

/// Reads a forbidden-words file from its text.
///
/// # Errors
///
/// Returns the parse error as text.
pub fn parse_forbidden(text: &str) -> Result<Forbidden, String> {
    serde_json::from_str(text).map_err(|error| error.to_string())
}

/// Renders a phrase in English with its placeholders filled.
///
/// A placeholder is `{name}`; each is replaced by the value given for that name. A placeholder left
/// unfilled stays visible in the output, which is a defect a test catches rather than a silent blank.
#[must_use]
pub fn render(phrase: Phrase, values: &[(&str, &str)]) -> String {
    let template = ENGLISH_CATALOGUE
        .as_ref()
        .ok()
        .and_then(|catalogue| catalogue.strings.get(phrase.key()))
        .map_or_else(|| format!("[{}]", phrase.key()), Clone::clone);

    values.iter().fold(template, |text, (name, value)| {
        text.replace(&format!("{{{name}}}"), value)
    })
}

/// Whether a text contains a forbidden word as a whole word, without regard to case.
#[must_use]
pub fn contains_forbidden_word(text: &str, word: &str) -> bool {
    let lowered = text.to_lowercase();
    let wanted = word.to_lowercase();
    let mut from = 0usize;

    while let Some(found) = lowered.get(from..).and_then(|rest| rest.find(&wanted)) {
        let start = from.saturating_add(found);
        let end = start.saturating_add(wanted.len());
        let before = lowered.get(..start).and_then(|s| s.chars().next_back());
        let after = lowered.get(end..).and_then(|s| s.chars().next());
        let bounded = |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '-');
        if bounded(before) && bounded(after) {
            return true;
        }
        from = start.saturating_add(1);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::{
        ENGLISH, ENGLISH_FORBIDDEN, Phrase, contains_forbidden_word, parse_catalogue, parse_forbidden, render,
    };
    use std::collections::BTreeSet;

    #[test]
    fn the_catalogue_holds_exactly_the_phrases_the_code_can_render() {
        // Arrange
        let catalogue = parse_catalogue(ENGLISH).unwrap();
        let in_code: BTreeSet<&str> = Phrase::ALL.iter().map(|phrase| phrase.key()).collect();
        let in_catalogue: BTreeSet<&str> = catalogue.strings.keys().map(String::as_str).collect();

        // Assert
        assert_eq!(in_code, in_catalogue);
        assert_eq!(catalogue.language, "en");
    }

    #[test]
    fn no_string_contains_a_forbidden_word() {
        // Arrange
        let catalogue = parse_catalogue(ENGLISH).unwrap();
        let forbidden = parse_forbidden(ENGLISH_FORBIDDEN).unwrap();

        // Act
        let offending: Vec<String> = catalogue
            .strings
            .iter()
            .flat_map(|(key, text)| {
                forbidden
                    .words
                    .iter()
                    .filter(|word| contains_forbidden_word(text, word))
                    .map(move |word| format!("{key}: {word}"))
            })
            .collect();

        // Assert
        assert!(forbidden.words.len() >= 10);
        assert!(offending.is_empty(), "{offending:?}");
    }

    #[test]
    fn a_placeholder_is_filled_and_an_unfilled_one_stays_visible() {
        // Act
        let filled = render(Phrase::FingerprintFile, &[("digest", "abc"), ("bytes", "12")]);
        let unfilled = render(Phrase::FingerprintFile, &[]);

        // Assert
        assert!(filled.contains("abc over 12 bytes"), "{filled}");
        assert!(unfilled.contains("{digest}"), "{unfilled}");
    }

    #[test]
    fn a_forbidden_word_is_matched_only_as_a_whole_word() {
        // Act and assert
        assert!(contains_forbidden_word("This file is Verified.", "verified"));
        assert!(!contains_forbidden_word(
            "says nothing about its authenticity",
            "authentic"
        ));
        assert!(!contains_forbidden_word("the verifier compares", "verified"));
        assert!(contains_forbidden_word("a tamper-proof seal", "tamper-proof"));
        assert!(!contains_forbidden_word("originally", "original"));
    }

    #[test]
    fn every_time_sentence_says_no_later_than_and_names_a_clock() {
        // Arrange
        let catalogue = parse_catalogue(ENGLISH).unwrap();

        // Act
        let sentences: Vec<(&String, &String)> = catalogue
            .strings
            .iter()
            .filter(|(key, _)| {
                key.starts_with("time.") && !key.contains(".note") && *key != "time.disagreement"
            })
            .collect();

        // Assert
        assert!(!sentences.is_empty());
        for (key, text) in sentences {
            assert!(text.contains("no later than"), "{key}: {text}");
            assert!(text.contains("{clock}"), "{key}: {text}");
        }
    }
}
