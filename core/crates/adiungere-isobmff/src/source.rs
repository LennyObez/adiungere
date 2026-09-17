//! Where bytes come from, and how many of them were asked for.
//!
//! Two abstractions, one algorithm. [`Source`] is synchronous and positional: a file, a slice, anything that
//! can hand back a range. [`AsyncSource`] is the same contract for a reader that has to wait, which is what a
//! browser reading a blob slice by slice is. Neither has a write capability, by construction: the type that
//! reads a recording cannot alter it.
//!
//! [`CountingSource`] wraps either and records every range read, so a test can state how many bytes an
//! inspection cost and prove that the media data box was never touched, rather than assume it.

use std::fmt;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// A failure to deliver bytes.
#[derive(Debug)]
pub enum SourceError {
    /// The operating system refused.
    Io(std::io::Error),
    /// A range was asked for that lies past the end of the source.
    OutOfBounds {
        /// The first byte asked for.
        offset: u64,
        /// How many bytes were asked for.
        length: u64,
        /// How long the source is.
        available: u64,
    },
}

impl fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(cause) => write!(formatter, "{cause}"),
            Self::OutOfBounds {
                offset,
                length,
                available,
            } => write!(
                formatter,
                "{length} bytes at {offset} were asked for and the source holds {available}"
            ),
        }
    }
}

impl std::error::Error for SourceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(cause) => Some(cause),
            Self::OutOfBounds { .. } => None,
        }
    }
}

impl From<std::io::Error> for SourceError {
    fn from(cause: std::io::Error) -> Self {
        Self::Io(cause)
    }
}

/// A positional, read-only byte source.
pub trait Source {
    /// The total length in bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the length cannot be determined.
    fn length(&mut self) -> Result<u64, SourceError>;

    /// Fills `into` with the bytes starting at `offset`, exactly, or fails.
    ///
    /// # Errors
    ///
    /// Returns an error when the range is not fully available.
    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError>;

    /// Reads a range into a fresh buffer.
    ///
    /// # Errors
    ///
    /// Returns an error when the range is not fully available.
    fn read_range(&mut self, offset: u64, length: usize) -> Result<Vec<u8>, SourceError> {
        // The bounds are checked before a byte is allocated. A hostile file can declare a sample of four
        // gigabytes in a file of forty kilobytes, and the allocation would be the damage.
        let available = self.length()?;
        let fits = offset
            .checked_add(length as u64)
            .is_some_and(|end| end <= available);
        if !fits {
            return Err(SourceError::OutOfBounds {
                offset,
                length: length as u64,
                available,
            });
        }
        let mut buffer = vec![0; length];
        self.read_at(offset, &mut buffer)?;
        Ok(buffer)
    }
}

/// A source over bytes already in memory.
#[derive(Debug)]
pub struct SliceSource<'a> {
    bytes: &'a [u8],
}

impl<'a> SliceSource<'a> {
    /// Wraps a slice.
    #[must_use]
    pub const fn new(bytes: &'a [u8]) -> Self {
        Self { bytes }
    }
}

impl Source for SliceSource<'_> {
    fn length(&mut self) -> Result<u64, SourceError> {
        Ok(self.bytes.len() as u64)
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        let length = into.len() as u64;
        let end = offset.checked_add(length);
        let available = self.bytes.len() as u64;

        match end {
            Some(end) if end <= available => {
                let start = usize::try_from(offset).map_err(|_| SourceError::OutOfBounds {
                    offset,
                    length,
                    available,
                })?;
                let slice = self.bytes.get(start..start.saturating_add(into.len())).ok_or(
                    SourceError::OutOfBounds {
                        offset,
                        length,
                        available,
                    },
                )?;
                into.copy_from_slice(slice);
                Ok(())
            },
            _ => Err(SourceError::OutOfBounds {
                offset,
                length,
                available,
            }),
        }
    }
}

/// A source over a file opened for reading only.
#[derive(Debug)]
pub struct FileSource {
    file: File,
    length: u64,
}

impl FileSource {
    /// Opens a file for reading. The handle has no write capability.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be opened or its length cannot be read.
    pub fn open(path: &Path) -> Result<Self, SourceError> {
        let file = File::open(path)?;
        let length = file.metadata()?.len();
        Ok(Self { file, length })
    }
}

impl Source for FileSource {
    fn length(&mut self) -> Result<u64, SourceError> {
        Ok(self.length)
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        let length = into.len() as u64;
        let within = offset.checked_add(length).is_some_and(|end| end <= self.length);

        if !within {
            return Err(SourceError::OutOfBounds {
                offset,
                length,
                available: self.length,
            });
        }

        self.file.seek(SeekFrom::Start(offset))?;
        self.file.read_exact(into)?;
        Ok(())
    }
}

/// One range that was read, as `[offset, offset + length)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadRange {
    /// The first byte.
    pub offset: u64,
    /// How many bytes.
    pub length: u64,
}

impl ReadRange {
    /// Whether this range shares at least one byte with `[start, end)`.
    #[must_use]
    pub fn overlaps(self, start: u64, end: u64) -> bool {
        let own_end = self.offset.saturating_add(self.length);
        self.offset < end && start < own_end
    }
}

/// A source that records every range read through it.
#[derive(Debug)]
pub struct CountingSource<S> {
    inner: S,
    reads: Vec<ReadRange>,
}

impl<S> CountingSource<S> {
    /// Wraps a source.
    #[must_use]
    pub const fn new(inner: S) -> Self {
        Self {
            inner,
            reads: Vec::new(),
        }
    }

    /// Every range read so far, in order.
    #[must_use]
    pub fn reads(&self) -> &[ReadRange] {
        &self.reads
    }

    /// The total number of bytes read so far.
    #[must_use]
    pub fn bytes_read(&self) -> u64 {
        self.reads
            .iter()
            .fold(0, |total, read| total.saturating_add(read.length))
    }

    /// Whether any read touched `[start, end)`.
    #[must_use]
    pub fn touched(&self, start: u64, end: u64) -> bool {
        self.reads.iter().any(|read| read.overlaps(start, end))
    }

    /// Gives the wrapped source back.
    pub fn into_inner(self) -> S {
        self.inner
    }
}

impl<S: Source> Source for CountingSource<S> {
    fn length(&mut self) -> Result<u64, SourceError> {
        self.inner.length()
    }

    fn read_at(&mut self, offset: u64, into: &mut [u8]) -> Result<(), SourceError> {
        self.reads.push(ReadRange {
            offset,
            length: into.len() as u64,
        });
        self.inner.read_at(offset, into)
    }
}

/// A positional, read-only byte source that has to wait for its bytes.
///
/// This is the shape of a browser reading a blob: each slice is a promise. The parsing algorithm is the
/// same one the synchronous driver runs, so the two cannot disagree.
pub trait AsyncSource {
    /// The total length in bytes.
    fn length(&mut self) -> impl Future<Output = Result<u64, SourceError>>;

    /// Reads exactly `length` bytes starting at `offset`.
    fn read_range(
        &mut self,
        offset: u64,
        length: usize,
    ) -> impl Future<Output = Result<Vec<u8>, SourceError>>;
}

/// Every synchronous source is trivially an asynchronous one whose futures are already complete. This is
/// how the asynchronous driver is tested against the synchronous one on the same bytes.
impl<S: Source> AsyncSource for CountingSource<S> {
    fn length(&mut self) -> impl Future<Output = Result<u64, SourceError>> {
        std::future::ready(Source::length(self))
    }

    fn read_range(
        &mut self,
        offset: u64,
        length: usize,
    ) -> impl Future<Output = Result<Vec<u8>, SourceError>> {
        std::future::ready(Source::read_range(self, offset, length))
    }
}

impl AsyncSource for SliceSource<'_> {
    fn length(&mut self) -> impl Future<Output = Result<u64, SourceError>> {
        std::future::ready(Source::length(self))
    }

    fn read_range(
        &mut self,
        offset: u64,
        length: usize,
    ) -> impl Future<Output = Result<Vec<u8>, SourceError>> {
        std::future::ready(Source::read_range(self, offset, length))
    }
}

#[cfg(test)]
mod tests {
    use super::{CountingSource, ReadRange, SliceSource, Source, SourceError};

    #[test]
    fn a_slice_hands_back_exactly_the_range_asked_for() {
        // Arrange
        let bytes = [1, 2, 3, 4, 5, 6];
        let mut source = SliceSource::new(&bytes);

        // Act
        let read = source.read_range(2, 3).unwrap();

        // Assert
        assert_eq!(read, vec![3, 4, 5]);
    }

    #[test]
    fn a_range_past_the_end_is_refused_rather_than_shortened() {
        // Arrange
        let bytes = [1, 2, 3];
        let mut source = SliceSource::new(&bytes);

        // Act
        let outcome = source.read_range(2, 2);

        // Assert
        assert!(
            matches!(
                outcome,
                Err(SourceError::OutOfBounds {
                    offset: 2,
                    length: 2,
                    available: 3
                })
            ),
            "got {outcome:?}"
        );
    }

    #[test]
    fn an_offset_that_overflows_is_refused() {
        // Arrange
        let bytes = [1, 2, 3];
        let mut source = SliceSource::new(&bytes);

        // Act
        let outcome = source.read_range(u64::MAX, 2);

        // Assert
        assert!(matches!(outcome, Err(SourceError::OutOfBounds { .. })));
    }

    #[test]
    fn the_counting_source_records_every_read_and_its_total() {
        // Arrange
        let bytes = [0; 100];
        let mut source = CountingSource::new(SliceSource::new(&bytes));

        // Act
        source.read_range(0, 8).unwrap();
        source.read_range(50, 10).unwrap();

        // Assert
        assert_eq!(
            source.reads(),
            &[
                ReadRange { offset: 0, length: 8 },
                ReadRange {
                    offset: 50,
                    length: 10
                }
            ]
        );
        assert_eq!(source.bytes_read(), 18);
        assert!(source.touched(55, 56));
        assert!(!source.touched(8, 50));
        assert!(!source.touched(60, 100));
    }

    #[test]
    fn overlap_is_half_open_on_both_sides() {
        // Arrange
        let read = ReadRange {
            offset: 10,
            length: 5,
        };

        // Act and assert
        assert!(read.overlaps(14, 20));
        assert!(!read.overlaps(15, 20));
        assert!(read.overlaps(0, 11));
        assert!(!read.overlaps(0, 10));
    }
}
