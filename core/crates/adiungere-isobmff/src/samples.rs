//! Locating samples: the tables that say where each one is stored, and the iterator that walks them in
//! decode order.
//!
//! Four tables locate a sample: how many samples each chunk holds, where each chunk starts, how large each
//! sample is, and how long each lasts. They are decoded here into one list, checked against each other,
//! and refused with a named inconsistency when they disagree. A fingerprint computed over samples that a
//! contradictory table placed wrongly would be a number nobody else could reproduce.

use serde::Serialize;

use crate::error::Error;
use crate::fourcc::FourCc;
use crate::parse::Container;
use crate::source::Source;
use crate::tree::BoxRange;
use crate::views::{Ctts, Stss, bounded_capacity};

/// The declared sizes of a track's samples.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Sizes {
    /// Every sample has this size, and there are this many.
    Constant {
        /// The size of every sample.
        size: u32,
        /// How many samples.
        count: u32,
    },
    /// Each sample has its own size.
    Each(Vec<u32>),
}

impl Sizes {
    /// How many samples the table describes.
    #[must_use]
    pub fn count(&self) -> u32 {
        match self {
            Self::Constant { count, .. } => *count,
            Self::Each(sizes) => u32::try_from(sizes.len()).unwrap_or(u32::MAX),
        }
    }

    /// The size of a sample by zero-based index.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<u32> {
        match self {
            Self::Constant { size, count } => {
                (index < usize::try_from(*count).unwrap_or(usize::MAX)).then_some(*size)
            },
            Self::Each(sizes) => sizes.get(index).copied(),
        }
    }
}

/// The tables of one track, decoded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SampleTable {
    /// The track identifier, for error messages.
    pub track_id: u32,
    /// Runs of `(sample count, duration in media ticks)`.
    pub time_to_sample: Vec<(u32, u32)>,
    /// Runs of `(first chunk, samples per chunk, sample description index)`, chunks one-based.
    pub sample_to_chunk: Vec<(u32, u32, u32)>,
    /// The sample sizes.
    pub sizes: Sizes,
    /// The absolute file offset of each chunk.
    pub chunk_offsets: Vec<u64>,
    /// Whether the chunk offsets were stored in the 64-bit table.
    pub wide_offsets: bool,
    /// The sync sample table, when the track has one. A track without one is all sync samples.
    pub sync: Option<Stss>,
    /// The composition offsets, when the track has any.
    pub composition: Option<Ctts>,
}

/// One sample, located.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Sample {
    /// The zero-based index in decode order.
    pub index: u32,
    /// The absolute file offset of the first byte.
    pub offset: u64,
    /// The size in bytes.
    pub size: u32,
    /// The decode time in media ticks.
    pub decode_time: u64,
    /// The duration in media ticks.
    pub duration: u32,
    /// The composition offset in media ticks, zero when the track has none.
    pub composition_offset: i64,
    /// Whether this is a sync sample.
    pub sync: bool,
    /// The one-based chunk the sample lies in.
    pub chunk: u32,
}

impl SampleTable {
    /// Reads and cross-checks the tables under a sample table box.
    ///
    /// # Errors
    ///
    /// Returns an error when a required table is missing, malformed, or contradicts another.
    pub fn read(container: &Container, stbl: &BoxRange, track_id: u32) -> Result<Self, Error> {
        let payload = |kind: &'static [u8; 4], field: &'static str| {
            let range = stbl.child(kind).ok_or(Error::Truncated {
                kind: stbl.kind,
                offset: stbl.offset,
                field,
            })?;
            Ok::<_, Error>((container.payload_of(range).unwrap_or(&[]), range.offset))
        };

        let (stts, stts_offset) = payload(b"stts", "time-to-sample table")?;
        let time_to_sample = read_pairs(stts, FourCc(*b"stts"), stts_offset)?;

        let (stsc, stsc_offset) = payload(b"stsc", "sample-to-chunk table")?;
        let sample_to_chunk = read_triples(stsc, FourCc(*b"stsc"), stsc_offset)?;

        let sizes = match (stbl.child(b"stsz"), stbl.child(b"stz2")) {
            (Some(range), _) => read_sizes(container.payload_of(range).unwrap_or(&[]), range.offset)?,
            (None, Some(range)) => {
                read_compact_sizes(container.payload_of(range).unwrap_or(&[]), range.offset)?
            },
            (None, None) => {
                return Err(Error::Truncated {
                    kind: stbl.kind,
                    offset: stbl.offset,
                    field: "sample size table",
                });
            },
        };

        let (chunk_offsets, wide_offsets) = match (stbl.child(b"co64"), stbl.child(b"stco")) {
            (Some(range), _) => (
                read_offsets(container.payload_of(range).unwrap_or(&[]), range.offset, true)?,
                true,
            ),
            (None, Some(range)) => (
                read_offsets(container.payload_of(range).unwrap_or(&[]), range.offset, false)?,
                false,
            ),
            (None, None) => {
                return Err(Error::Truncated {
                    kind: stbl.kind,
                    offset: stbl.offset,
                    field: "chunk offset table",
                });
            },
        };

        let sync = match stbl.child(b"stss") {
            Some(range) => Some(Stss::read(
                container.payload_of(range).unwrap_or(&[]),
                range.offset,
            )?),
            None => None,
        };
        let composition = match stbl.child(b"ctts") {
            Some(range) => Some(Ctts::read(
                container.payload_of(range).unwrap_or(&[]),
                range.offset,
            )?),
            None => None,
        };

        let table = Self {
            track_id,
            time_to_sample,
            sample_to_chunk,
            sizes,
            chunk_offsets,
            wide_offsets,
            sync,
            composition,
        };
        table.check()?;
        Ok(table)
    }

    /// How many samples the size table declares.
    #[must_use]
    pub fn sample_count(&self) -> u32 {
        self.sizes.count()
    }

    /// The sum of every sample's size.
    #[must_use]
    pub fn total_sample_bytes(&self) -> u64 {
        match &self.sizes {
            Sizes::Constant { size, count } => u64::from(*size).saturating_mul(u64::from(*count)),
            Sizes::Each(sizes) => sizes
                .iter()
                .fold(0u64, |total, size| total.saturating_add(u64::from(*size))),
        }
    }

    /// How many sync samples the track has: the sync table's length, or every sample without a table.
    #[must_use]
    pub fn sync_sample_count(&self) -> u32 {
        self.sync.as_ref().map_or(self.sample_count(), |stss| {
            u32::try_from(stss.sample_numbers.len()).unwrap_or(u32::MAX)
        })
    }

    /// Whether the composition table is present and not trivially zero.
    #[must_use]
    pub fn has_composition_offsets(&self) -> bool {
        self.composition
            .as_ref()
            .is_some_and(|ctts| ctts.entries.iter().any(|(_, offset)| *offset != 0))
    }

    fn inconsistent(&self, what: &'static str) -> Error {
        Error::Inconsistent {
            track: self.track_id,
            what,
        }
    }

    /// The cross-checks. Each names what disagreed.
    fn check(&self) -> Result<(), Error> {
        let count = self.sample_count();

        let timed: u64 = self
            .time_to_sample
            .iter()
            .fold(0u64, |total, (n, _)| total.saturating_add(u64::from(*n)));
        if timed != u64::from(count) {
            return Err(self.inconsistent(
                "the time-to-sample table covers a different number of samples than the size table",
            ));
        }

        let chunk_count = u32::try_from(self.chunk_offsets.len()).unwrap_or(u32::MAX);
        let mut previous_first: Option<u32> = None;
        for (first, per_chunk, _) in &self.sample_to_chunk {
            let ascending = previous_first.is_none_or(|previous| *first > previous);
            if *first == 0 || !ascending {
                return Err(self.inconsistent("the sample-to-chunk table is not ascending from chunk one"));
            }
            if *first > chunk_count {
                return Err(
                    self.inconsistent("the sample-to-chunk table names a chunk past the offset table")
                );
            }
            if *per_chunk == 0 {
                return Err(self.inconsistent("the sample-to-chunk table declares an empty chunk"));
            }
            previous_first = Some(*first);
        }
        if count > 0 && (self.sample_to_chunk.is_empty() || self.chunk_offsets.is_empty()) {
            return Err(self.inconsistent("samples are declared and no chunk locates them"));
        }
        if self
            .sample_to_chunk
            .first()
            .is_some_and(|(first, _, _)| *first != 1)
        {
            return Err(self.inconsistent("the sample-to-chunk table does not start at chunk one"));
        }

        let placed = self.samples_in_chunks();
        if placed != u64::from(count) {
            return Err(self
                .inconsistent("the chunks hold a different number of samples than the size table declares"));
        }

        if let Some(stss) = &self.sync {
            let mut previous = 0u32;
            for number in &stss.sample_numbers {
                if *number == 0 || *number <= previous || *number > count {
                    return Err(
                        self.inconsistent("the sync sample table is not ascending within the sample count")
                    );
                }
                previous = *number;
            }
        }

        if let Some(ctts) = &self.composition {
            let covered = ctts
                .entries
                .iter()
                .fold(0u64, |total, (n, _)| total.saturating_add(u64::from(*n)));
            if covered != u64::from(count) {
                return Err(self.inconsistent(
                    "the composition offset table covers a different number of samples than the size table",
                ));
            }
        }

        Ok(())
    }

    /// How many samples the chunk runs place, given the number of chunks.
    fn samples_in_chunks(&self) -> u64 {
        let chunk_count = u64::from(u32::try_from(self.chunk_offsets.len()).unwrap_or(u32::MAX));
        let mut total = 0u64;

        for (position, (first, per_chunk, _)) in self.sample_to_chunk.iter().enumerate() {
            let next_first = self
                .sample_to_chunk
                .get(position.saturating_add(1))
                .map_or(chunk_count.saturating_add(1), |(next, _, _)| u64::from(*next));
            let chunks = next_first.saturating_sub(u64::from(*first));
            total = total.saturating_add(chunks.saturating_mul(u64::from(*per_chunk)));
        }

        total
    }

    /// Every sample in decode order, located.
    ///
    /// # Errors
    ///
    /// Returns an error when an offset overflows, which the cross-checks cannot rule out in advance.
    pub fn samples(&self) -> Result<Vec<Sample>, Error> {
        let count = self.sample_count();
        let mut samples = Vec::with_capacity(bounded_capacity(count));

        let mut durations = self
            .time_to_sample
            .iter()
            .flat_map(|(n, delta)| std::iter::repeat_n(*delta, usize::try_from(*n).unwrap_or(0)));
        let mut compositions = self.composition.as_ref().map(|ctts| {
            ctts.entries
                .iter()
                .flat_map(|(n, offset)| std::iter::repeat_n(*offset, usize::try_from(*n).unwrap_or(0)))
        });
        let sync_numbers = self.sync.as_ref().map(|stss| stss.sample_numbers.as_slice());

        let mut decode_time = 0u64;
        let mut index = 0u32;
        let chunk_count = u32::try_from(self.chunk_offsets.len()).unwrap_or(u32::MAX);

        for (position, (first, per_chunk, _)) in self.sample_to_chunk.iter().enumerate() {
            let next_first = self
                .sample_to_chunk
                .get(position.saturating_add(1))
                .map_or(chunk_count.saturating_add(1), |(next, _, _)| *next);

            for chunk in *first..next_first {
                let chunk_index = usize::try_from(chunk.saturating_sub(1)).unwrap_or(usize::MAX);
                let mut offset = *self
                    .chunk_offsets
                    .get(chunk_index)
                    .ok_or_else(|| self.inconsistent("a chunk run names a chunk past the offset table"))?;

                for _ in 0..*per_chunk {
                    let size = self
                        .sizes
                        .get(usize::try_from(index).unwrap_or(usize::MAX))
                        .ok_or_else(|| self.inconsistent("more samples are placed than sized"))?;
                    let duration = durations.next().unwrap_or(0);
                    let composition_offset = compositions.as_mut().and_then(Iterator::next).unwrap_or(0);
                    let sync = sync_numbers
                        .is_none_or(|numbers| numbers.binary_search(&index.saturating_add(1)).is_ok());

                    samples.push(Sample {
                        index,
                        offset,
                        size,
                        decode_time,
                        duration,
                        composition_offset,
                        sync,
                        chunk,
                    });

                    offset = offset
                        .checked_add(u64::from(size))
                        .ok_or_else(|| self.inconsistent("a sample offset overflows"))?;
                    decode_time = decode_time
                        .checked_add(u64::from(duration))
                        .ok_or_else(|| self.inconsistent("a decode time overflows"))?;
                    index = index.saturating_add(1);
                }
            }
        }

        Ok(samples)
    }

    /// Reads one sample's stored bytes.
    ///
    /// # Errors
    ///
    /// Returns an error when the source cannot deliver the range.
    pub fn read_sample<S: Source>(source: &mut S, sample: &Sample) -> Result<Vec<u8>, Error> {
        if sample.size > MAX_SAMPLE_SIZE {
            return Err(Error::Inconsistent {
                track: 0,
                what: "a sample is larger than any this reader holds",
            });
        }
        let length = usize::try_from(sample.size).map_err(|_| Error::Inconsistent {
            track: 0,
            what: "a sample is larger than this platform can hold",
        })?;
        Ok(source.read_range(sample.offset, length)?)
    }
}

/// The largest sample the reader holds in memory at once. A coded picture at the highest resolution and
/// bit rate a dashcam produces is a few megabytes; a table that declares more is corrupt or hostile.
pub const MAX_SAMPLE_SIZE: u32 = 64 * 1024 * 1024;

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    fn u32(&mut self) -> Option<u32> {
        let slice = self.bytes.get(self.at..self.at.checked_add(4)?)?;
        self.at = self.at.saturating_add(4);
        Some(u32::from_be_bytes(slice.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        let slice = self.bytes.get(self.at..self.at.checked_add(8)?)?;
        self.at = self.at.saturating_add(8);
        Some(u64::from_be_bytes(slice.try_into().ok()?))
    }
}

fn truncated(kind: FourCc, offset: u64, field: &'static str) -> Error {
    Error::Truncated { kind, offset, field }
}

fn read_pairs(payload: &[u8], kind: FourCc, offset: u64) -> Result<Vec<(u32, u32)>, Error> {
    let mut reader = Reader {
        bytes: payload,
        at: 4,
    };
    let count = reader.u32().ok_or(truncated(kind, offset, "entry count"))?;
    let mut entries = Vec::with_capacity(bounded_capacity(count));
    for _ in 0..count {
        let first = reader.u32().ok_or(truncated(kind, offset, "entry"))?;
        let second = reader.u32().ok_or(truncated(kind, offset, "entry"))?;
        entries.push((first, second));
    }
    Ok(entries)
}

fn read_triples(payload: &[u8], kind: FourCc, offset: u64) -> Result<Vec<(u32, u32, u32)>, Error> {
    let mut reader = Reader {
        bytes: payload,
        at: 4,
    };
    let count = reader.u32().ok_or(truncated(kind, offset, "entry count"))?;
    let mut entries = Vec::with_capacity(bounded_capacity(count));
    for _ in 0..count {
        let first = reader.u32().ok_or(truncated(kind, offset, "entry"))?;
        let second = reader.u32().ok_or(truncated(kind, offset, "entry"))?;
        let third = reader.u32().ok_or(truncated(kind, offset, "entry"))?;
        entries.push((first, second, third));
    }
    Ok(entries)
}

fn read_sizes(payload: &[u8], offset: u64) -> Result<Sizes, Error> {
    let kind = FourCc(*b"stsz");
    let mut reader = Reader {
        bytes: payload,
        at: 4,
    };
    let size = reader.u32().ok_or(truncated(kind, offset, "sample size"))?;
    let count = reader.u32().ok_or(truncated(kind, offset, "sample count"))?;

    if size != 0 {
        return Ok(Sizes::Constant { size, count });
    }

    let mut sizes = Vec::with_capacity(bounded_capacity(count));
    for _ in 0..count {
        sizes.push(reader.u32().ok_or(truncated(kind, offset, "entry"))?);
    }
    Ok(Sizes::Each(sizes))
}

fn read_compact_sizes(payload: &[u8], offset: u64) -> Result<Sizes, Error> {
    let kind = FourCc(*b"stz2");
    let field_size = payload
        .get(7)
        .copied()
        .ok_or(truncated(kind, offset, "field size"))?;
    let mut reader = Reader {
        bytes: payload,
        at: 8,
    };
    let count = reader.u32().ok_or(truncated(kind, offset, "sample count"))?;
    let entries = payload.get(12..).unwrap_or(&[]);
    let mut sizes = Vec::with_capacity(bounded_capacity(count));

    for index in 0..usize::try_from(count).unwrap_or(0) {
        let size = match field_size {
            4 => {
                let byte = entries
                    .get(index.checked_div(2).unwrap_or(0))
                    .copied()
                    .ok_or(truncated(kind, offset, "entry"))?;
                if index.checked_rem(2) == Some(0) {
                    u32::from(byte >> 4)
                } else {
                    u32::from(byte & 0x0f)
                }
            },
            8 => u32::from(
                entries
                    .get(index)
                    .copied()
                    .ok_or(truncated(kind, offset, "entry"))?,
            ),
            16 => {
                let start = index.checked_mul(2).ok_or(truncated(kind, offset, "entry"))?;
                let slice = entries
                    .get(start..start.saturating_add(2))
                    .ok_or(truncated(kind, offset, "entry"))?;
                u32::from(u16::from_be_bytes(slice.try_into().unwrap_or([0; 2])))
            },
            other => {
                return Err(Error::UnknownVersion {
                    kind,
                    offset,
                    version: other,
                });
            },
        };
        sizes.push(size);
    }

    Ok(Sizes::Each(sizes))
}

fn read_offsets(payload: &[u8], offset: u64, wide: bool) -> Result<Vec<u64>, Error> {
    let kind = FourCc(if wide { *b"co64" } else { *b"stco" });
    let mut reader = Reader {
        bytes: payload,
        at: 4,
    };
    let count = reader.u32().ok_or(truncated(kind, offset, "entry count"))?;
    let mut offsets = Vec::with_capacity(bounded_capacity(count));
    for _ in 0..count {
        let value = if wide {
            reader.u64()
        } else {
            reader.u32().map(u64::from)
        };
        offsets.push(value.ok_or(truncated(kind, offset, "entry"))?);
    }
    Ok(offsets)
}

#[cfg(test)]
mod tests {
    use super::{MAX_SAMPLE_SIZE, Sample, SampleTable, Sizes, read_compact_sizes, read_offsets, read_sizes};
    use crate::error::Error;
    use crate::views::{Ctts, Stss};

    fn table(sizes: Vec<u32>, chunks: Vec<u64>, runs: Vec<(u32, u32, u32)>) -> SampleTable {
        let count = u32::try_from(sizes.len()).unwrap();
        SampleTable {
            track_id: 1,
            time_to_sample: vec![(count, 2000)],
            sample_to_chunk: runs,
            sizes: Sizes::Each(sizes),
            chunk_offsets: chunks,
            wide_offsets: false,
            sync: None,
            composition: None,
        }
    }

    #[test]
    fn samples_are_placed_consecutively_within_each_chunk() {
        // Arrange
        let table = table(vec![10, 20, 30, 40], vec![100, 500], vec![(1, 2, 1)]);

        // Act
        let samples = table.samples().unwrap();

        // Assert
        let offsets: Vec<u64> = samples.iter().map(|sample| sample.offset).collect();
        assert_eq!(offsets, vec![100, 110, 500, 530]);
        let times: Vec<u64> = samples.iter().map(|sample| sample.decode_time).collect();
        assert_eq!(times, vec![0, 2000, 4000, 6000]);
        assert!(samples.iter().all(|sample| sample.sync));
    }

    #[test]
    fn the_last_chunk_run_extends_to_the_last_chunk() {
        // Arrange
        let table = table(vec![1, 1, 1, 1, 1], vec![0, 10, 20], vec![(1, 1, 1), (2, 2, 1)]);

        // Act
        let samples = table.samples().unwrap();

        // Assert
        let chunks: Vec<u32> = samples.iter().map(|sample| sample.chunk).collect();
        assert_eq!(chunks, vec![1, 2, 2, 3, 3]);
    }

    #[test]
    fn a_chunk_run_that_places_a_different_count_than_the_sizes_is_refused() {
        // Arrange
        let table = table(vec![1, 1, 1], vec![0, 10], vec![(1, 2, 1)]);

        // Act
        let outcome = table.check();

        // Assert
        assert!(
            matches!(outcome, Err(Error::Inconsistent { track: 1, what }) if what.contains("chunks hold")),
            "got {outcome:?}"
        );
    }

    #[test]
    fn a_chunk_run_past_the_offset_table_is_refused() {
        // Arrange
        let table = table(vec![1, 1], vec![0], vec![(1, 1, 1), (2, 1, 1)]);

        // Act
        let outcome = table.check();

        // Assert
        assert!(
            matches!(outcome, Err(Error::Inconsistent { .. })),
            "got {outcome:?}"
        );
    }

    #[test]
    fn a_timing_table_covering_the_wrong_count_is_refused() {
        // Arrange
        let mut table = table(vec![1, 1], vec![0], vec![(1, 2, 1)]);
        table.time_to_sample = vec![(3, 1)];

        // Act
        let outcome = table.check();

        // Assert
        assert!(
            matches!(outcome, Err(Error::Inconsistent { what, .. }) if what.contains("time-to-sample")),
            "got {outcome:?}"
        );
    }

    #[test]
    fn an_offset_that_would_overflow_is_an_error_rather_than_a_wrap() {
        // Arrange
        let table = table(vec![u32::MAX, 1], vec![u64::MAX - 1], vec![(1, 2, 1)]);

        // Act
        let outcome = table.samples();

        // Assert
        assert!(
            matches!(outcome, Err(Error::Inconsistent { .. })),
            "got {outcome:?}"
        );
    }

    #[test]
    fn a_chunk_run_may_name_the_last_chunk_and_not_one_past_it() {
        // Arrange
        let last = table(vec![1, 1], vec![0, 10], vec![(1, 1, 1), (2, 1, 1)]);
        let past = table(vec![1, 1], vec![0, 10], vec![(1, 1, 1), (3, 1, 1)]);
        let backwards = table(vec![1, 1], vec![0, 10], vec![(2, 1, 1), (2, 1, 1)]);
        let empty_chunk = table(vec![1, 1], vec![0, 10], vec![(1, 0, 1), (2, 2, 1)]);
        let not_from_one = table(vec![1], vec![0, 10], vec![(2, 1, 1)]);

        // Act and assert
        assert!(last.check().is_ok());
        assert!(matches!(past.check(), Err(Error::Inconsistent { what, .. }) if what.contains("past")));
        assert!(
            matches!(backwards.check(), Err(Error::Inconsistent { what, .. }) if what.contains("ascending"))
        );
        assert!(
            matches!(empty_chunk.check(), Err(Error::Inconsistent { what, .. }) if what.contains("empty"))
        );
        assert!(
            matches!(not_from_one.check(), Err(Error::Inconsistent { what, .. }) if what.contains("chunk one"))
        );
    }

    #[test]
    fn a_track_with_no_sample_and_no_chunk_is_consistent_and_empty() {
        // Arrange
        let mut table = table(vec![], vec![], vec![]);
        table.time_to_sample = Vec::new();

        // Act
        let checked = table.check();
        let samples = table.samples().unwrap();

        // Assert
        assert!(checked.is_ok());
        assert!(samples.is_empty());
        assert_eq!(table.total_sample_bytes(), 0);
        assert_eq!(table.sync_sample_count(), 0);
    }

    #[test]
    fn the_sample_cap_is_sixty_four_mebibytes() {
        // Act and assert
        assert_eq!(MAX_SAMPLE_SIZE, 67_108_864);
    }

    #[test]
    fn samples_declared_without_a_chunk_are_refused() {
        // Arrange
        let neither = table(vec![1], vec![], vec![]);
        let offsets_without_runs = table(vec![1], vec![0], vec![]);

        // Act
        let outcome = neither.check();
        let half = offsets_without_runs.check();

        // Assert
        assert!(matches!(outcome, Err(Error::Inconsistent { what, .. }) if what.contains("no chunk")));
        assert!(matches!(half, Err(Error::Inconsistent { what, .. }) if what.contains("no chunk")));
    }

    #[test]
    fn a_sync_table_may_name_the_last_sample_and_nothing_beyond_or_twice() {
        // Arrange
        let mut last = table(vec![1, 1, 1], vec![0], vec![(1, 3, 1)]);
        last.sync = Some(Stss {
            sample_numbers: vec![1, 3],
        });
        let mut beyond = last.clone();
        beyond.sync = Some(Stss {
            sample_numbers: vec![1, 4],
        });
        let mut twice = last.clone();
        twice.sync = Some(Stss {
            sample_numbers: vec![2, 2],
        });
        let mut zero = last.clone();
        zero.sync = Some(Stss {
            sample_numbers: vec![0],
        });

        // Act and assert
        assert!(last.check().is_ok());
        assert_eq!(last.sync_sample_count(), 2);
        assert!(beyond.check().is_err());
        assert!(twice.check().is_err());
        assert!(zero.check().is_err());
        let samples = last.samples().unwrap();
        let sync: Vec<bool> = samples.iter().map(|sample| sample.sync).collect();
        assert_eq!(sync, vec![true, false, true]);
    }

    #[test]
    fn a_composition_table_must_cover_every_sample_and_is_trivial_when_all_zero() {
        // Arrange
        let mut covered = table(vec![1, 1], vec![0], vec![(1, 2, 1)]);
        covered.composition = Some(Ctts {
            entries: vec![(2, 0)],
        });
        let mut short = covered.clone();
        short.composition = Some(Ctts {
            entries: vec![(1, 0)],
        });
        let mut shifted = covered.clone();
        shifted.composition = Some(Ctts {
            entries: vec![(1, 0), (1, 1000)],
        });

        // Act and assert
        assert!(covered.check().is_ok());
        assert!(!covered.has_composition_offsets());
        assert!(short.check().is_err());
        assert!(shifted.check().is_ok());
        assert!(shifted.has_composition_offsets());
        let offsets: Vec<i64> = shifted
            .samples()
            .unwrap()
            .iter()
            .map(|sample| sample.composition_offset)
            .collect();
        assert_eq!(offsets, vec![0, 1000]);
    }

    #[test]
    fn totals_are_summed_over_both_size_forms() {
        // Arrange
        let each = table(vec![10, 20, 30], vec![0], vec![(1, 3, 1)]);
        let mut constant = each.clone();
        constant.sizes = Sizes::Constant { size: 7, count: 3 };

        // Act and assert
        assert_eq!(each.total_sample_bytes(), 60);
        assert_eq!(constant.total_sample_bytes(), 21);
        assert_eq!(constant.sample_count(), 3);
        assert_eq!(constant.sizes.get(2), Some(7));
        assert_eq!(constant.sizes.get(3), None);
        assert_eq!(each.sizes.get(2), Some(30));
        assert_eq!(each.sizes.get(3), None);
        assert_eq!(
            each.sync_sample_count(),
            3,
            "a track without a sync table is all sync samples"
        );
    }

    #[test]
    fn a_sample_larger_than_the_reader_holds_is_refused_before_it_is_read() {
        // Arrange
        let bytes = [0u8; 16];
        let mut source = crate::source::SliceSource::new(&bytes);
        let huge = Sample {
            index: 0,
            offset: 0,
            size: MAX_SAMPLE_SIZE + 1,
            decode_time: 0,
            duration: 0,
            composition_offset: 0,
            sync: true,
            chunk: 1,
        };
        let large = Sample {
            size: MAX_SAMPLE_SIZE,
            ..huge
        };
        let small = Sample { size: 4, ..huge };

        // Act
        let refused = SampleTable::read_sample(&mut source, &huge);
        let out_of_bounds = SampleTable::read_sample(&mut source, &large);
        let read = SampleTable::read_sample(&mut source, &small).unwrap();

        // Assert
        assert!(matches!(refused, Err(Error::Inconsistent { what, .. }) if what.contains("larger")));
        assert!(matches!(out_of_bounds, Err(Error::Source(_))));
        assert_eq!(read, vec![0; 4]);
    }

    #[test]
    fn the_compact_size_table_is_read_at_each_field_width() {
        // Arrange: version and flags, three reserved bytes, the field size, the count, then the entries.
        let four = [0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 3, 0x12, 0x30];
        let eight = [0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 2, 200, 7];
        let sixteen = [0, 0, 0, 0, 0, 0, 0, 16, 0, 0, 0, 1, 0x01, 0x00];
        let odd = [0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 1, 0];

        // Act
        let narrow = read_compact_sizes(&four, 0).unwrap();
        let bytes = read_compact_sizes(&eight, 0).unwrap();
        let wide = read_compact_sizes(&sixteen, 0).unwrap();
        let unknown = read_compact_sizes(&odd, 0);
        let short = read_compact_sizes(&four[..13], 0);

        // Assert
        assert_eq!(narrow, Sizes::Each(vec![1, 2, 3]));
        assert_eq!(bytes, Sizes::Each(vec![200, 7]));
        assert_eq!(wide, Sizes::Each(vec![256]));
        assert!(matches!(unknown, Err(Error::UnknownVersion { version: 3, .. })));
        assert!(matches!(short, Err(Error::Truncated { .. })));
    }

    #[test]
    fn the_offset_tables_are_read_in_both_widths() {
        // Arrange
        let mut narrow = vec![0, 0, 0, 0];
        narrow.extend_from_slice(&2u32.to_be_bytes());
        narrow.extend_from_slice(&40u32.to_be_bytes());
        narrow.extend_from_slice(&50u32.to_be_bytes());
        let mut wide = vec![0, 0, 0, 0];
        wide.extend_from_slice(&1u32.to_be_bytes());
        wide.extend_from_slice(&(1u64 << 33).to_be_bytes());

        // Act
        let stco = read_offsets(&narrow, 0, false).unwrap();
        let co64 = read_offsets(&wide, 0, true).unwrap();
        let short = read_offsets(&narrow[..10], 0, false);

        // Assert
        assert_eq!(stco, vec![40, 50]);
        assert_eq!(co64, vec![1 << 33]);
        assert!(matches!(short, Err(Error::Truncated { .. })));
    }

    #[test]
    fn a_constant_size_table_and_a_per_sample_table_are_told_apart() {
        // Arrange
        let mut constant = vec![0, 0, 0, 0];
        constant.extend_from_slice(&512u32.to_be_bytes());
        constant.extend_from_slice(&9u32.to_be_bytes());
        let mut each = vec![0, 0, 0, 0];
        each.extend_from_slice(&0u32.to_be_bytes());
        each.extend_from_slice(&2u32.to_be_bytes());
        each.extend_from_slice(&5u32.to_be_bytes());
        each.extend_from_slice(&6u32.to_be_bytes());

        // Act
        let fixed = read_sizes(&constant, 0).unwrap();
        let listed = read_sizes(&each, 0).unwrap();

        // Assert
        assert_eq!(fixed, Sizes::Constant { size: 512, count: 9 });
        assert_eq!(listed, Sizes::Each(vec![5, 6]));
    }
}
