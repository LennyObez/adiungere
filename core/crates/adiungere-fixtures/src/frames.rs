//! The committed frames, read back into access units.
//!
//! Three small streams were encoded once and committed: two seconds of synthetic video pattern, one per
//! camera, as raw elementary streams with start codes, and one second of a sine tone as audio transport
//! frames. Nothing here decodes or encodes; the streams are split into the units a container stores, and
//! the parameter sets a decoder configuration carries are lifted out of the video streams.

/// The front camera's elementary stream, as committed.
pub const FRONT: &[u8] = include_bytes!("../../../fixtures/frames/front.h264");
/// The rear camera's elementary stream, as committed.
pub const REAR: &[u8] = include_bytes!("../../../fixtures/frames/rear.h264");
/// The audio transport stream, as committed.
pub const AUDIO: &[u8] = include_bytes!("../../../fixtures/frames/audio.aac");

/// A video stream split into access units, with its parameter sets.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VideoStream {
    /// Each access unit as the list of its units, without start codes, in order.
    pub access_units: Vec<AccessUnit>,
    /// The sequence parameter sets, first occurrence of each, as stored.
    pub sequence_parameter_sets: Vec<Vec<u8>>,
    /// The picture parameter sets, first occurrence of each, as stored.
    pub picture_parameter_sets: Vec<Vec<u8>>,
}

/// One coded picture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessUnit {
    /// The slice units, in order, each without a start code.
    pub units: Vec<Vec<u8>>,
    /// Whether the picture is an instantaneous decoder refresh.
    pub sync: bool,
}

/// Why a committed stream could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FrameError {
    /// The stream has no start code where one was expected.
    NoStartCode,
    /// The stream holds no picture.
    NoPictures,
    /// The stream holds no parameter set of the named kind.
    NoParameterSet(&'static str),
    /// An audio frame header is not a transport header.
    NotTransport {
        /// Where the frame begins.
        offset: usize,
    },
    /// An audio frame declares a length that runs past the stream.
    FrameOverrun {
        /// Where the frame begins.
        offset: usize,
    },
}

impl std::fmt::Display for FrameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoStartCode => formatter.write_str("the stream does not begin with a start code"),
            Self::NoPictures => formatter.write_str("the stream holds no picture"),
            Self::NoParameterSet(kind) => write!(formatter, "the stream holds no {kind}"),
            Self::NotTransport { offset } => {
                write!(formatter, "no transport header at {offset}")
            },
            Self::FrameOverrun { offset } => {
                write!(formatter, "the frame at {offset} runs past the stream")
            },
        }
    }
}

impl std::error::Error for FrameError {}

/// Splits a stream with start codes into its units, each without the code and without a trailing zero
/// that belongs to the next four-byte code.
fn units_of(stream: &[u8]) -> Result<Vec<&[u8]>, FrameError> {
    let mut starts = Vec::new();
    let mut at = 0usize;

    while let Some(found) = find(stream, at, &[0, 0, 1]) {
        starts.push(found);
        at = found.saturating_add(3);
    }

    if starts.first() != Some(&0) && starts.first() != Some(&1) {
        return Err(FrameError::NoStartCode);
    }

    let mut units = Vec::with_capacity(starts.len());

    for (position, start) in starts.iter().enumerate() {
        let begin = start.saturating_add(3);
        let mut end = starts
            .get(position.saturating_add(1))
            .copied()
            .unwrap_or(stream.len());
        // The zero that opens a four-byte start code is not part of the preceding unit.
        if end > begin && stream.get(end.wrapping_sub(1)) == Some(&0) {
            end = end.saturating_sub(1);
        }
        units.push(stream.get(begin..end).unwrap_or(&[]));
    }

    Ok(units)
}

fn find(haystack: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    haystack
        .get(from..)?
        .windows(needle.len())
        .position(|window| window == needle)
        .map(|position| position.saturating_add(from))
}

/// Reads a video elementary stream into access units and parameter sets.
///
/// Supplemental enhancement units are dropped: they carry the encoder's own signature and nothing a
/// decoder needs, and a fixture should not carry a tool's name.
///
/// # Errors
///
/// Returns an error when the stream is not a start-coded stream or holds no picture.
pub fn video(stream: &[u8]) -> Result<VideoStream, FrameError> {
    let mut sequence_parameter_sets: Vec<Vec<u8>> = Vec::new();
    let mut picture_parameter_sets: Vec<Vec<u8>> = Vec::new();
    let mut access_units: Vec<AccessUnit> = Vec::new();

    for unit in units_of(stream)? {
        let unit_type = unit.first().copied().unwrap_or(0) & 0x1f;

        match unit_type {
            7 => {
                if !sequence_parameter_sets.iter().any(|set| set == unit) {
                    sequence_parameter_sets.push(unit.to_vec());
                }
            },
            8 => {
                if !picture_parameter_sets.iter().any(|set| set == unit) {
                    picture_parameter_sets.push(unit.to_vec());
                }
            },
            1 | 5 => {
                // A slice whose first macroblock is zero opens a new picture. The field is the first
                // Exp-Golomb value after the unit header, and zero is coded as a single one bit.
                let opens_picture = unit.get(1).is_some_and(|byte| byte & 0x80 != 0);
                if opens_picture || access_units.is_empty() {
                    access_units.push(AccessUnit {
                        units: Vec::new(),
                        sync: unit_type == 5,
                    });
                }
                if let Some(current) = access_units.last_mut() {
                    current.units.push(unit.to_vec());
                    current.sync |= unit_type == 5;
                }
            },
            _ => {},
        }
    }

    if access_units.is_empty() {
        return Err(FrameError::NoPictures);
    }
    if sequence_parameter_sets.is_empty() {
        return Err(FrameError::NoParameterSet("sequence parameter set"));
    }
    if picture_parameter_sets.is_empty() {
        return Err(FrameError::NoParameterSet("picture parameter set"));
    }

    Ok(VideoStream {
        access_units,
        sequence_parameter_sets,
        picture_parameter_sets,
    })
}

/// An audio stream split into raw frames, with what a decoder configuration needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioStream {
    /// Each frame without its transport header.
    pub frames: Vec<Vec<u8>>,
    /// The audio object type from the transport header, one-based.
    pub object_type: u8,
    /// The sampling frequency index from the transport header.
    pub frequency_index: u8,
    /// The channel configuration from the transport header.
    pub channel_configuration: u8,
}

impl AudioStream {
    /// The sampling rate the frequency index stands for, or zero when the index is reserved.
    #[must_use]
    pub fn sample_rate(&self) -> u32 {
        const RATES: [u32; 13] = [
            96000, 88200, 64000, 48000, 44100, 32000, 24000, 22050, 16000, 12000, 11025, 8000, 7350,
        ];
        RATES.get(usize::from(self.frequency_index)).copied().unwrap_or(0)
    }

    /// The two-byte audio specific configuration a decoder is handed.
    #[must_use]
    pub fn specific_configuration(&self) -> [u8; 2] {
        let first = (self.object_type << 3) | (self.frequency_index >> 1);
        let second = ((self.frequency_index & 1) << 7) | (self.channel_configuration << 3);
        [first, second]
    }
}

/// Reads an audio transport stream into raw frames.
///
/// # Errors
///
/// Returns an error when a header is not a transport header or a frame overruns the stream.
pub fn audio(stream: &[u8]) -> Result<AudioStream, FrameError> {
    let mut frames = Vec::new();
    let mut at = 0usize;
    let mut object_type = 0;
    let mut frequency_index = 0;
    let mut channel_configuration = 0;

    while at < stream.len() {
        let header = stream
            .get(at..at.saturating_add(7))
            .ok_or(FrameError::NotTransport { offset: at })?;
        let sync_word = header.first().copied().unwrap_or(0) == 0xff
            && header.get(1).copied().unwrap_or(0) & 0xf6 == 0xf0;
        if !sync_word {
            return Err(FrameError::NotTransport { offset: at });
        }

        let protection_absent = header.get(1).copied().unwrap_or(0) & 1 == 1;
        let byte2 = header.get(2).copied().unwrap_or(0);
        let byte3 = header.get(3).copied().unwrap_or(0);
        let byte4 = header.get(4).copied().unwrap_or(0);
        let byte5 = header.get(5).copied().unwrap_or(0);

        object_type = (byte2 >> 6).saturating_add(1);
        frequency_index = (byte2 >> 2) & 0x0f;
        channel_configuration = ((byte2 & 1) << 2) | (byte3 >> 6);

        let length = (usize::from(byte3 & 3) << 11) | (usize::from(byte4) << 3) | usize::from(byte5 >> 5);
        let header_length = if protection_absent { 7 } else { 9 };
        let end = at.saturating_add(length);
        let payload = stream
            .get(at.saturating_add(header_length)..end)
            .ok_or(FrameError::FrameOverrun { offset: at })?;

        frames.push(payload.to_vec());
        at = end;
    }

    if frames.is_empty() {
        return Err(FrameError::NoPictures);
    }

    Ok(AudioStream {
        frames,
        object_type,
        frequency_index,
        channel_configuration,
    })
}

#[cfg(test)]
mod tests {
    use super::{AUDIO, FRONT, FrameError, REAR, audio, video};

    #[test]
    fn the_front_stream_reads_as_thirty_pictures_with_two_key_frames() {
        // Act
        let stream = video(FRONT).unwrap();

        // Assert
        assert_eq!(stream.access_units.len(), 30);
        let sync: Vec<usize> = stream
            .access_units
            .iter()
            .enumerate()
            .filter(|(_, unit)| unit.sync)
            .map(|(index, _)| index)
            .collect();
        assert_eq!(sync, vec![0, 15]);
        assert_eq!(stream.sequence_parameter_sets.len(), 1);
        assert_eq!(stream.picture_parameter_sets.len(), 1);
    }

    #[test]
    fn a_key_frame_holds_several_slices_and_every_unit_fits_one_byte_of_length() {
        // The encoder was asked for slices under two hundred bytes, so that every prefix width the
        // container allows can be exercised, including one byte.

        // Act
        let stream = video(FRONT).unwrap();

        // Assert
        let first = stream.access_units.first().unwrap();
        assert!(first.units.len() > 1, "the key frame is one slice");
        let widest = stream
            .access_units
            .iter()
            .flat_map(|unit| unit.units.iter())
            .map(Vec::len)
            .max()
            .unwrap();
        assert!(
            widest < 256,
            "a unit of {widest} bytes does not fit a one-byte length"
        );
    }

    #[test]
    fn the_rear_stream_differs_from_the_front_one() {
        // Act
        let front = video(FRONT).unwrap();
        let rear = video(REAR).unwrap();

        // Assert
        assert_ne!(front.access_units, rear.access_units);
        assert_eq!(rear.access_units.len(), 30);
    }

    #[test]
    fn no_supplemental_unit_survives_into_an_access_unit() {
        // Act
        let stream = video(FRONT).unwrap();

        // Assert
        let types: Vec<u8> = stream
            .access_units
            .iter()
            .flat_map(|unit| unit.units.iter())
            .map(|unit| unit.first().copied().unwrap_or(0) & 0x1f)
            .collect();
        assert!(types.iter().all(|kind| *kind == 1 || *kind == 5), "got {types:?}");
    }

    #[test]
    fn the_audio_stream_reads_as_low_complexity_stereo_at_forty_four_kilohertz() {
        // Act
        let stream = audio(AUDIO).unwrap();

        // Assert
        assert_eq!(stream.frames.len(), 45);
        assert_eq!(stream.object_type, 2);
        assert_eq!(stream.sample_rate(), 44100);
        assert_eq!(stream.channel_configuration, 2);
        assert_eq!(stream.specific_configuration(), [0x12, 0x10]);
    }

    #[test]
    fn a_stream_without_a_start_code_is_refused() {
        // Act
        let outcome = video(&[1, 2, 3, 4]);

        // Assert
        assert_eq!(outcome.unwrap_err(), FrameError::NoStartCode);
    }

    #[test]
    fn a_transport_frame_that_overruns_is_refused() {
        // Arrange
        let mut stream = AUDIO.to_vec();
        stream.truncate(20);

        // Act
        let outcome = audio(&stream);

        // Assert
        assert_eq!(outcome.unwrap_err(), FrameError::FrameOverrun { offset: 0 });
    }
}
