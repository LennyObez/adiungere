//! Reading one file just far enough to say what it is.
//!
//! The probe reads the headers and the movie box, never the media, so scanning a library of thousands of
//! recordings costs a few hundred kilobytes per file rather than gigabytes. What it learns is enough to
//! group files into recordings and to notice the signs of a rewrite.

use std::path::Path;

use adiungere_isobmff::{CountingSource, FileSource, Source, TrackKind, parse};
use adiungere_manifest::time::Civil;
use serde::{Deserialize, Serialize};

use crate::grammar::{ParsedName, parse_name};

/// What the movie box says about a file, in the few numbers a scan needs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSummary {
    /// How many video tracks.
    pub video_tracks: u32,
    /// How many audio tracks.
    pub audio_tracks: u32,
    /// How many video tracks carry the enabled flag.
    pub enabled_video_tracks: u32,
    /// Whether the movie box precedes the media data box.
    pub moov_before_mdat: Option<bool>,
    /// How many vendor boxes were found, under the user-data box and at the top level.
    pub vendor_boxes: u32,
    /// The compressor names and handler names, without duplicates.
    pub encoder_tags: Vec<String>,
    /// The coded width of the first video track.
    pub width: Option<u16>,
    /// The coded height of the first video track.
    pub height: Option<u16>,
    /// The duration of the longest track in milliseconds.
    pub duration_milliseconds: Option<u64>,
    /// The container clock, as written.
    pub container_time: Option<Civil>,
    /// Any tag found in the user-data bytes that names another program's muxer.
    pub foreign_muxer_tags: Vec<String>,
}

/// What a probe learned about one file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Probe {
    /// The file name, without its directory.
    pub name: String,
    /// The size in bytes.
    pub size: u64,
    /// What the name says, when it fits a grammar.
    pub parsed: Option<ParsedName>,
    /// What the container says, when the file could be read as one.
    pub container: Option<ContainerSummary>,
    /// Why the container could not be read, when it could not.
    pub unreadable: Option<String>,
    /// How many bytes the probe read.
    pub bytes_read: u64,
}

/// Byte strings that another program's muxer writes into the user-data box. Each was measured in a file
/// that program produced, and the list grows only from measurements.
const FOREIGN_MUXER_TAGS: [&[u8]; 2] = [b"Lavf", b"Lavc"];

/// Probes a file on disk.
///
/// # Errors
///
/// Returns an error only when the file cannot be opened or its size cannot be read; a file that opens
/// and is not a container is a probe with `unreadable` set, because that is a finding about the file.
pub fn probe_file(path: &Path) -> Result<Probe, std::io::Error> {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    let size = std::fs::metadata(path)?.len();
    let file = FileSource::open(path).map_err(|error| match error {
        adiungere_isobmff::SourceError::Io(cause) => cause,
        other @ adiungere_isobmff::SourceError::OutOfBounds { .. } => {
            std::io::Error::other(other.to_string())
        },
    })?;
    let mut source = CountingSource::new(file);
    let probe = probe_source(&mut source, &name, size);
    Ok(Probe {
        bytes_read: source.bytes_read(),
        ..probe
    })
}

/// Probes any source, given the name the file carries.
pub fn probe_source<S: Source>(source: &mut S, name: &str, size: u64) -> Probe {
    let parsed = parse_name(name);
    let (container, unreadable) = match summarise(source) {
        Ok(summary) => (Some(summary), None),
        Err(error) => (None, Some(error)),
    };
    Probe {
        name: name.to_owned(),
        size,
        parsed,
        container,
        unreadable,
        bytes_read: 0,
    }
}

fn summarise<S: Source>(source: &mut S) -> Result<ContainerSummary, String> {
    let container = parse(source).map_err(|error| error.to_string())?;
    let tracks = container.tracks().map_err(|error| error.to_string())?;

    let mut encoder_tags: Vec<String> = Vec::new();
    for track in &tracks {
        let tags = track
            .entry()
            .and_then(|entry| entry.visual.as_ref())
            .map(|visual| visual.compressor_name.clone())
            .into_iter()
            .chain(std::iter::once(track.hdlr.name.clone()));
        for tag in tags {
            if !tag.is_empty() && !encoder_tags.contains(&tag) {
                encoder_tags.push(tag);
            }
        }
    }

    let first_video = tracks.iter().find(|track| track.kind == TrackKind::Video);
    let visual = first_video
        .and_then(|track| track.entry())
        .and_then(|entry| entry.visual.clone());

    let mut foreign_muxer_tags = Vec::new();
    for range in container.udta_children() {
        if let Some(bytes) = container.bytes_of(range) {
            for tag in FOREIGN_MUXER_TAGS {
                if bytes.windows(tag.len()).any(|window| window == tag) {
                    let text = String::from_utf8_lossy(tag).into_owned();
                    if !foreign_muxer_tags.contains(&text) {
                        foreign_muxer_tags.push(text);
                    }
                }
            }
        }
    }

    Ok(ContainerSummary {
        video_tracks: count(&tracks, |track| track.kind == TrackKind::Video),
        audio_tracks: count(&tracks, |track| track.kind == TrackKind::Audio),
        enabled_video_tracks: count(&tracks, |track| {
            track.kind == TrackKind::Video && track.tkhd.is_enabled()
        }),
        moov_before_mdat: container.moov_before_mdat(),
        vendor_boxes: count_vendor_boxes(&container),
        encoder_tags,
        width: visual.as_ref().map(|visual| visual.width),
        height: visual.as_ref().map(|visual| visual.height),
        duration_milliseconds: tracks
            .iter()
            .filter_map(adiungere_isobmff::Track::duration_milliseconds)
            .max(),
        container_time: container
            .mvhd()
            .ok()
            .map(|mvhd| Civil::from_container_seconds(mvhd.creation_time)),
        foreign_muxer_tags,
    })
}

/// The boxes a recorder, not a standard, put there: the children of the user-data box whose type no
/// standard defines, and the top-level boxes the reader does not know. A metadata box a common muxer
/// writes under user data is a standard one and is not counted.
fn count_vendor_boxes(container: &adiungere_isobmff::Container) -> u32 {
    let under_user_data = container
        .udta_children()
        .iter()
        .filter(|range| !range.kind.is_standard_user_data())
        .count();
    u32::try_from(under_user_data + container.unknown_top_level().len()).unwrap_or(u32::MAX)
}

fn count(tracks: &[adiungere_isobmff::Track], wanted: impl Fn(&adiungere_isobmff::Track) -> bool) -> u32 {
    u32::try_from(tracks.iter().filter(|track| wanted(track)).count()).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use adiungere_fixtures::{Spec, VENDOR_BOX, build, corpus};
    use adiungere_isobmff::SliceSource;

    use super::probe_source;

    #[test]
    fn the_summary_of_the_reference_recording_holds_every_number_the_scan_uses() {
        // Arrange
        let mut fixture = build(&Spec::reference_like()).unwrap();
        let length = fixture.length();

        // Act
        let probe = probe_source(&mut fixture, "20260604_122323E.MP4", length);
        let summary = probe.container.unwrap();

        // Assert
        assert!(probe.unreadable.is_none());
        assert!(probe.parsed.is_some());
        assert_eq!(probe.size, fixture.length());
        assert_eq!(summary.video_tracks, 2);
        assert_eq!(summary.audio_tracks, 1);
        assert_eq!(summary.enabled_video_tracks, 2);
        assert_eq!(summary.moov_before_mdat, Some(true));
        assert_eq!(
            summary.vendor_boxes, 2,
            "one under user data, one at the top level"
        );
        assert_eq!(
            summary.encoder_tags,
            vec![
                "synthetic pattern".to_owned(),
                "VideoHandler".to_owned(),
                "SoundHandler".to_owned()
            ],
            "each tag once, in track order, the empty ones left out"
        );
        assert_eq!(summary.width, Some(64));
        assert_eq!(summary.height, Some(64));
        assert_eq!(
            summary.duration_milliseconds,
            Some(1044),
            "the longest track: 45 audio frames of 1024 samples at 44100 Hz"
        );
        assert_eq!(
            summary.container_time.unwrap().to_string(),
            adiungere_manifest::time::Civil::from_container_seconds(adiungere_fixtures::CREATION_TIME)
                .to_string()
        );
        assert!(summary.foreign_muxer_tags.is_empty());
    }

    #[test]
    fn a_rewritten_recording_is_summarised_with_its_marks() {
        // Arrange
        let spec = corpus()
            .into_iter()
            .find(|spec| spec.name == "rewritten")
            .unwrap();
        let mut fixture = build(&spec).unwrap();
        let length = fixture.length();

        // Act
        let summary = probe_source(&mut fixture, "x.mp4", length).container.unwrap();

        // Assert
        assert_eq!(summary.video_tracks, 1);
        assert_eq!(summary.enabled_video_tracks, 1);
        assert_eq!(summary.moov_before_mdat, Some(false));
        assert_eq!(summary.vendor_boxes, 0);
        assert!(summary.encoder_tags.contains(&"another program".to_owned()));
    }

    #[test]
    fn a_user_data_box_of_a_standard_type_is_not_a_vendor_box_and_a_foreign_tag_in_it_is_found() {
        // Arrange: the vendor box becomes a metadata box carrying the tag a common muxer writes.
        let fixture = build(&Spec::reference_like()).unwrap();
        let mut bytes = fixture.bytes().unwrap();
        let header = bytes.windows(4).position(|window| window == VENDOR_BOX).unwrap();
        bytes
            .get_mut(header..header + 4)
            .unwrap()
            .copy_from_slice(b"meta");
        bytes
            .get_mut(header + 4..header + 8)
            .unwrap()
            .copy_from_slice(b"Lavf");
        let mut source = SliceSource::new(&bytes);

        // Act
        let summary = probe_source(&mut source, "x.mp4", fixture.length())
            .container
            .unwrap();

        // Assert
        assert_eq!(
            summary.vendor_boxes, 1,
            "only the top-level box remains a vendor's"
        );
        assert_eq!(summary.foreign_muxer_tags, vec!["Lavf".to_owned()]);
    }

    #[test]
    fn a_file_that_is_not_a_container_is_a_probe_that_says_why() {
        // Arrange
        let bytes = b"not a container at all, just text";
        let mut source = SliceSource::new(bytes);

        // Act
        let probe = probe_source(&mut source, "20260604_122323E.MP4", 33);

        // Assert
        assert!(probe.container.is_none());
        assert!(probe.parsed.is_some(), "the name still says what it says");
        assert!(probe.unreadable.unwrap().contains("box header"));
    }
}
