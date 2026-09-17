//! Rewriting a container around samples that do not change.
//!
//! An extraction keeps some of a recording's tracks and drops the others; an archive joins the tracks of
//! two recordings into one. Neither touches a coded sample: every byte of media in the output is a byte
//! of media in an input, in the same order within its track. What is rebuilt is the container: a movie
//! box that lists the kept tracks, chunk offset tables that point into the new media data box, and
//! nothing else. Every other box is copied as the bytes it was read as, which is how the recorder's own
//! boxes, the user-data children and the unknown top-level boxes survive.
//!
//! The output is deterministic: the same inputs and the same plan produce the same bytes on every
//! platform, because nothing here reads a clock or depends on allocation order. The movie box precedes
//! the media data box, so the output plays before it has finished arriving. Media is copied chunk by chunk
//! through a bounded buffer, so the memory needed does not grow with the recording.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;

use crate::error::Error;
use crate::fourcc::FourCc;
use crate::parse::Container;
use crate::samples::Sample;
use crate::source::Source;
use crate::tree::BoxRange;
use crate::views::Track;

/// One recording taking part in a remux, with the tracks to keep from it.
pub struct Input<'a> {
    /// The recording, read.
    pub container: &'a Container,
    /// Where its bytes come from.
    pub source: &'a mut dyn Source,
    /// The tracks to keep, by index under the movie box, in output order.
    pub tracks: Vec<usize>,
}

impl std::fmt::Debug for Input<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Input")
            .field("tracks", &self.tracks)
            .finish_non_exhaustive()
    }
}

/// What to carry across besides the tracks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RemuxPlan {
    /// Whether the user-data box of the first input is copied as bytes.
    pub keep_udta: bool,
    /// Whether the unknown top-level boxes of the first input are copied as bytes, after the movie box.
    pub keep_unknown_top_level: bool,
    /// Whether a manifest store the first input carries is copied like any other top-level box. Off by
    /// default, because a store is bound to the bytes of the file it was written into and is wrong in
    /// any other; on, it produces exactly the file a rewriter that does not know the standard produces,
    /// which is what a test of the validators needs.
    pub keep_manifest_store: bool,
}

impl Default for RemuxPlan {
    fn default() -> Self {
        Self {
            keep_udta: true,
            keep_unknown_top_level: true,
            keep_manifest_store: false,
        }
    }
}

/// How far a remux has come. Handed to the progress callback after every chunk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Progress {
    /// Bytes written so far, headers included.
    pub bytes_written: u64,
    /// Bytes the whole output will hold.
    pub bytes_total: u64,
}

/// Where a preserved box sits in the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// A child of the user-data box under the movie box.
    UserData,
    /// A top-level box after the movie box.
    TopLevel,
}

/// A box copied as bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreservedBox {
    /// Its type code.
    pub kind: FourCc,
    /// Its size, header included.
    pub size: u64,
    /// Where it sits.
    pub placement: Placement,
}

/// One track of the output and where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemuxedTrack {
    /// The input, by position.
    pub input: usize,
    /// The track's index under the input's movie box.
    pub source_index: usize,
    /// The track's index under the output's movie box.
    pub output_index: usize,
    /// The track identifier in the output.
    pub track_id: u32,
    /// The track identifier in the input, which differs only when two inputs were joined.
    pub source_track_id: u32,
    /// How many samples were copied.
    pub samples: u32,
    /// How many media bytes were copied.
    pub bytes: u64,
}

/// What a remux did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemuxReport {
    /// How many bytes the output holds.
    pub bytes_written: u64,
    /// Every track, in output order.
    pub tracks: Vec<RemuxedTrack>,
    /// Every box copied as bytes, in output order.
    pub preserved: Vec<PreservedBox>,
    /// Whether the chunk offsets were written in the 64-bit table, which happens when the media ends past
    /// four gibibytes.
    pub wide_offsets: bool,
    /// How many chunks the media data box holds.
    pub chunks: u32,
    /// Whether the track identifiers were renumbered, which happens when two inputs are joined.
    pub renumbered: bool,
    /// How many track references pointed at a track the output does not hold and were dropped, so that
    /// nothing in the output refers to a track that is not there.
    pub references_dropped: u32,
    /// Whether the primary input carried a manifest store, which is bound to the bytes of the file it was
    /// written into and is therefore left out of a rewritten one rather than carried across as a box.
    pub credentials_left_out: bool,
}

/// One track selected for the output, with everything the layout needs.
struct Selected<'a> {
    input: usize,
    container: &'a Container,
    track: Track,
    samples: Vec<Sample>,
    track_id: u32,
}

/// A run of consecutive samples of one track that the input stores together, and the output stores
/// together too.
#[derive(Debug, Clone, Copy)]
struct Chunk {
    slot: usize,
    first: usize,
    count: usize,
    bytes: u64,
    time: u64,
    timescale: u32,
}

/// The most media read at once while copying. A chunk larger than this is copied sample by sample.
const COPY_BUFFER: u64 = 16 * 1024 * 1024;

/// Rewrites the selected tracks of the inputs into `out`.
///
/// The first input is the primary: its file type box, its movie header, its user-data box and its
/// unknown top-level boxes are the ones copied. A single input keeps its track identifiers; joined inputs
/// are renumbered from one in output order. The callback is told the progress after every chunk and
/// stops the remux by returning false.
///
/// # Errors
///
/// Returns an error when a selected track does not exist or is selected twice, when nothing is selected,
/// when a box to copy was not held by the reader, when a source or the output fails, or when the
/// callback stops the remux.
pub fn remux(
    inputs: &mut [Input<'_>],
    plan: &RemuxPlan,
    out: &mut dyn Write,
    progress: &mut dyn FnMut(&Progress) -> bool,
) -> Result<RemuxReport, Error> {
    let primary = inputs.first().ok_or(Error::NothingSelected)?.container;

    let mut selected = select(inputs)?;
    let renumbered = renumber(&mut selected, inputs.len());
    let chunks = interleave(&selected);
    let media: u64 = chunks.iter().map(|chunk| chunk.bytes).sum();

    // The fixed parts of the output, before the movie box is laid out.
    let ftyp = primary
        .top_level()
        .iter()
        .find(|range| range.kind == b"ftyp")
        .map(|range| held(primary, range))
        .transpose()?;
    // A manifest store is not a recorder's box: bound to the bytes of the file it was written into, it
    // can only be wrong in a rewritten one, so it is left out and reported rather than carried across.
    let mut unknown: Vec<&BoxRange> = Vec::new();
    let mut credentials_left_out = false;
    for range in primary.unknown_top_level() {
        if !plan.keep_manifest_store && is_manifest_store(inputs, range)? {
            credentials_left_out = true;
        } else if plan.keep_unknown_top_level {
            unknown.push(range);
        }
    }
    let before_moov = ftyp.map_or(0, |bytes| bytes.len() as u64);
    let after_moov: u64 = unknown.iter().map(|range| range.size).sum();
    let large_mdat = needs_large_header(media);
    let mdat_header: u64 = if large_mdat { 16 } else { 8 };
    let ids = identifiers(&selected);
    let layout = Layout {
        primary,
        selected: &selected,
        chunks: &chunks,
        ids: &ids,
        keep_udta: plan.keep_udta,
        renumbered,
    };

    // The movie box is laid out twice at most: once to learn its size with narrow offsets, and again
    // with wide ones if the media would end past what narrow offsets can address. The offsets do not
    // change the size of the movie box, only their width does.
    let media_start = |moov_len: u64| before_moov + moov_len + after_moov + mdat_header;
    let mut wide = false;
    let narrow_moov = movie_box(&layout, wide, 0)?;
    if !fits_narrow_offsets(media_start(narrow_moov.len() as u64).saturating_add(media)) {
        wide = true;
    }
    let sized_moov = movie_box(&layout, wide, 0)?;
    let base = media_start(sized_moov.len() as u64);
    let moov = movie_box(&layout, wide, base)?;
    let references_dropped = layout.references_dropped()?;

    let bytes_total = base + media;
    let mut written = Counting { inner: out, bytes: 0 };

    if let Some(bytes) = ftyp {
        written.write_all(bytes)?;
    }
    written.write_all(&moov)?;
    for range in &unknown {
        copy_box(inputs, primary, range, &mut written)?;
    }
    written.write_all(&box_header(*b"mdat", media, large_mdat))?;

    for chunk in &chunks {
        copy_chunk(inputs, &selected, chunk, &mut written)?;
        let state = Progress {
            bytes_written: written.bytes,
            bytes_total,
        };
        if !progress(&state) {
            return Err(Error::Cancelled {
                bytes_written: written.bytes,
            });
        }
    }
    written.flush()?;

    Ok(RemuxReport {
        bytes_written: written.bytes,
        tracks: remuxed_tracks(&selected),
        preserved: preserved_boxes(primary, plan.keep_udta, &unknown),
        wide_offsets: wide,
        chunks: u32::try_from(chunks.len()).unwrap_or(u32::MAX),
        renumbered,
        references_dropped,
        credentials_left_out,
    })
}

/// The extended type of the box the provenance standard reserves for a manifest store.
const MANIFEST_STORE: [u8; 16] = [
    0xd8, 0xfe, 0xc3, 0xd6, 0x1b, 0x0e, 0x48, 0x3c, 0x92, 0x97, 0x58, 0x28, 0x87, 0x7e, 0xc4, 0x81,
];

/// Whether a top-level box of the primary input is a manifest store: a `uuid` box whose extended type is
/// the reserved one, read from the source so that a store larger than the reader holds is seen too.
fn is_manifest_store(inputs: &mut [Input<'_>], range: &BoxRange) -> Result<bool, Error> {
    if range.kind != b"uuid" || range.size < 24 {
        return Ok(false);
    }
    let input = inputs.first_mut().ok_or(Error::NothingSelected)?;
    let extended = input
        .source
        .read_range(range.offset.saturating_add(8), MANIFEST_STORE.len())?;
    Ok(extended == MANIFEST_STORE)
}

/// The boxes carried across as bytes, in output order.
fn preserved_boxes(primary: &Container, keep_udta: bool, unknown: &[&BoxRange]) -> Vec<PreservedBox> {
    let mut preserved = Vec::new();
    if keep_udta {
        for child in primary.udta_children() {
            preserved.push(PreservedBox {
                kind: child.kind,
                size: child.size,
                placement: Placement::UserData,
            });
        }
    }
    for range in unknown {
        preserved.push(PreservedBox {
            kind: range.kind,
            size: range.size,
            placement: Placement::TopLevel,
        });
    }
    preserved
}

/// Every kept track as the report describes it, in output order.
fn remuxed_tracks(selected: &[Selected<'_>]) -> Vec<RemuxedTrack> {
    selected
        .iter()
        .enumerate()
        .map(|(output_index, slot)| RemuxedTrack {
            input: slot.input,
            source_index: slot.track.index,
            output_index,
            track_id: slot.track_id,
            source_track_id: slot.track.tkhd.track_id,
            samples: u32::try_from(slot.samples.len()).unwrap_or(u32::MAX),
            bytes: slot.samples.iter().map(|sample| u64::from(sample.size)).sum(),
        })
        .collect()
}

/// Copies one top-level box of the primary input to the output: from the bytes the reader held when it
/// held them, and otherwise from the source in bounded pieces, so a large unknown box is carried across
/// without being read whole.
fn copy_box(
    inputs: &mut [Input<'_>],
    primary: &Container,
    range: &BoxRange,
    out: &mut dyn Write,
) -> Result<(), Error> {
    if let Some(bytes) = primary.bytes_of(range) {
        out.write_all(bytes)?;
        return Ok(());
    }
    let input = inputs.first_mut().ok_or(Error::NothingSelected)?;
    let mut offset = range.offset;
    let end = range.offset.saturating_add(range.size);
    while offset < end {
        let piece = usize::try_from((end - offset).min(COPY_BUFFER)).map_err(|_| Error::NotHeld {
            kind: range.kind,
            offset: range.offset,
        })?;
        let bytes = input.source.read_range(offset, piece)?;
        out.write_all(&bytes)?;
        offset = offset.saturating_add(piece as u64);
    }
    Ok(())
}

/// The identifier every kept track has in the output, by input and by the identifier it had there.
fn identifiers(selected: &[Selected<'_>]) -> BTreeMap<(usize, u32), u32> {
    selected
        .iter()
        .map(|slot| ((slot.input, slot.track.tkhd.track_id), slot.track_id))
        .collect()
}

/// What the movie box is laid out from: the same inputs for every pass.
struct Layout<'a, 'c> {
    primary: &'c Container,
    selected: &'a [Selected<'c>],
    chunks: &'a [Chunk],
    ids: &'a BTreeMap<(usize, u32), u32>,
    keep_udta: bool,
    renumbered: bool,
}

impl Layout<'_, '_> {
    /// How many track references point at a track the output does not hold, and are therefore dropped.
    fn references_dropped(&self) -> Result<u32, Error> {
        let mut dropped = 0u32;
        for item in self.selected {
            for child in &item.track.range.children {
                if child.kind == b"tref" {
                    dropped = dropped.saturating_add(references_box(item, child, self.ids)?.1);
                }
            }
        }
        Ok(dropped)
    }
}

/// The track reference box of a kept track, rewritten for the output: every reference to a kept track
/// carries that track's output identifier, every reference to a track the output does not hold is
/// dropped, a reference type left with no target is dropped, and a box left with no reference type is
/// dropped altogether. Returns the box, if one remains, and how many references were dropped.
fn references_box(
    item: &Selected<'_>,
    tref: &BoxRange,
    ids: &BTreeMap<(usize, u32), u32>,
) -> Result<(Option<Vec<u8>>, u32), Error> {
    let mut payload = Vec::new();
    let mut dropped = 0u32;
    for reference in &tref.children {
        let bytes = held(item.container, reference)?;
        let targets = bytes.get(usize::from(reference.header)..).unwrap_or(&[]);
        let mut kept = Vec::new();
        for target in targets.as_chunks::<4>().0 {
            let source_id = u32::from_be_bytes(*target);
            match ids.get(&(item.input, source_id)) {
                Some(new_id) => kept.extend_from_slice(&new_id.to_be_bytes()),
                None => dropped = dropped.saturating_add(1),
            }
        }
        if !kept.is_empty() {
            payload.extend_from_slice(&boxed(reference.kind.bytes(), &kept));
        }
    }
    let remaining = (!payload.is_empty()).then(|| boxed(*b"tref", &payload));
    Ok((remaining, dropped))
}

/// A writer that counts what passes through it.
struct Counting<'a> {
    inner: &'a mut dyn Write,
    bytes: u64,
}

impl Write for Counting<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        let written = self.inner.write(buffer)?;
        self.bytes = self.bytes.saturating_add(written as u64);
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.inner.flush()
    }
}

/// The bytes of a box the reader held, or the error that says it did not.
fn held<'c>(container: &'c Container, range: &BoxRange) -> Result<&'c [u8], Error> {
    container.bytes_of(range).ok_or(Error::NotHeld {
        kind: range.kind,
        offset: range.offset,
    })
}

/// Resolves every selected track of every input, in output order.
fn select<'a>(inputs: &[Input<'a>]) -> Result<Vec<Selected<'a>>, Error> {
    let mut selected = Vec::new();
    for (position, input) in inputs.iter().enumerate() {
        let tracks = input.container.tracks()?;
        let mut chosen = BTreeSet::new();
        for &index in &input.tracks {
            let track = tracks.get(index).filter(|_| chosen.insert(index)).cloned();
            let track = track.ok_or(Error::NoSuchTrack {
                input: position,
                index,
            })?;
            let samples = track.table.samples()?;
            let track_id = track.tkhd.track_id;
            selected.push(Selected {
                input: position,
                container: input.container,
                track,
                samples,
                track_id,
            });
        }
    }
    if selected.is_empty() {
        return Err(Error::NothingSelected);
    }
    Ok(selected)
}

/// Gives every track a distinct identifier. A single input keeps its identifiers, so its track headers
/// are copied untouched; joined inputs are renumbered from one in output order, whether or not their
/// identifiers happen to collide, so the result does not depend on what each recorder chose.
fn renumber(selected: &mut [Selected<'_>], inputs: usize) -> bool {
    if inputs < 2 {
        return false;
    }
    for (position, slot) in selected.iter_mut().enumerate() {
        slot.track_id = u32::try_from(position).unwrap_or(u32::MAX).saturating_add(1);
    }
    true
}

/// Every chunk of every selected track, in the order the output stores them: by the decode time of the
/// first sample, then by track, then by position within the track. A chunk is the run of samples the
/// input stored together, so the recorder's own interleaving is what the output carries.
fn interleave(selected: &[Selected<'_>]) -> Vec<Chunk> {
    let mut chunks = Vec::new();
    for (slot, item) in selected.iter().enumerate() {
        let timescale = item.track.mdhd.timescale;
        let mut start = 0usize;
        while let Some(first) = item.samples.get(start) {
            let rest = item.samples.get(start..).unwrap_or(&[]);
            let count = rest
                .iter()
                .take_while(|sample| sample.chunk == first.chunk)
                .count()
                .max(1);
            let bytes = rest.iter().take(count).map(|sample| u64::from(sample.size)).sum();
            chunks.push(Chunk {
                slot,
                first: start,
                count,
                bytes,
                time: first.decode_time,
                timescale,
            });
            start = start.saturating_add(count);
        }
    }
    // The sort is stable, so ties keep track order, then chunk order.
    chunks.sort_by(earlier);
    chunks
}

/// Which of two chunks the output stores first: the one whose first sample decodes earlier, then the one
/// of the earlier track. Times are compared as fractions, so tracks with different timescales interleave
/// exactly and no rounding decides an order.
fn earlier(a: &Chunk, b: &Chunk) -> std::cmp::Ordering {
    let left = u128::from(a.time) * u128::from(b.timescale.max(1));
    let right = u128::from(b.time) * u128::from(a.timescale.max(1));
    left.cmp(&right).then(a.slot.cmp(&b.slot))
}

/// The largest position a narrow, 32-bit, size or offset can hold.
const NARROW_LIMIT: u64 = 0xFFFF_FFFF;

/// Whether a box with this payload needs the long header, because its size would not fit the short one.
const fn needs_large_header(payload: u64) -> bool {
    payload.saturating_add(8) > NARROW_LIMIT
}

/// Whether media ending at this position can be addressed by narrow chunk offsets.
const fn fits_narrow_offsets(end: u64) -> bool {
    end <= NARROW_LIMIT
}

/// Copies one chunk from its input to the output, through a bounded buffer.
fn copy_chunk(
    inputs: &mut [Input<'_>],
    selected: &[Selected<'_>],
    chunk: &Chunk,
    out: &mut dyn Write,
) -> Result<(), Error> {
    let slot = selected.get(chunk.slot).ok_or(Error::NothingSelected)?;
    let input = inputs.get_mut(slot.input).ok_or(Error::NothingSelected)?;
    let run = slot
        .samples
        .get(chunk.first..chunk.first.saturating_add(chunk.count))
        .ok_or(Error::NothingSelected)?;
    let too_large = |what: &'static str| Error::Inconsistent {
        track: slot.track_id,
        what,
    };

    // Samples stored back to back are read as one range; anything else, sample by sample.
    let contiguous = run.windows(2).all(|pair| {
        pair.first()
            .zip(pair.get(1))
            .is_some_and(|(a, b)| a.offset.saturating_add(u64::from(a.size)) == b.offset)
    });
    if contiguous
        && chunk.bytes <= COPY_BUFFER
        && let Some(first) = run.first()
    {
        let length = usize::try_from(chunk.bytes)
            .map_err(|_| too_large("a chunk is larger than this platform can hold"))?;
        let bytes = input.source.read_range(first.offset, length)?;
        out.write_all(&bytes)?;
        return Ok(());
    }
    for sample in run {
        let length = usize::try_from(sample.size)
            .map_err(|_| too_large("a sample is larger than this platform can hold"))?;
        let bytes = input.source.read_range(sample.offset, length)?;
        out.write_all(&bytes)?;
    }
    Ok(())
}

/// A box header for a payload of the given length, in the long form when asked.
fn box_header(kind: [u8; 4], payload: u64, large: bool) -> Vec<u8> {
    let mut header = Vec::with_capacity(16);
    if large {
        header.extend_from_slice(&1u32.to_be_bytes());
        header.extend_from_slice(&kind);
        header.extend_from_slice(&payload.saturating_add(16).to_be_bytes());
    } else {
        let size = u32::try_from(payload.saturating_add(8)).unwrap_or(u32::MAX);
        header.extend_from_slice(&size.to_be_bytes());
        header.extend_from_slice(&kind);
    }
    header
}

/// A whole box: header and payload.
fn boxed(kind: [u8; 4], payload: &[u8]) -> Vec<u8> {
    let large = needs_large_header(payload.len() as u64);
    let mut bytes = box_header(kind, payload.len() as u64, large);
    bytes.extend_from_slice(payload);
    bytes
}

/// The movie box of the output: the primary's children in their order, with the kept tracks written
/// where the primary's first track stood, the dropped tracks left out, and the user-data box kept or not
/// as the plan says.
fn movie_box(layout: &Layout<'_, '_>, wide: bool, media_start: u64) -> Result<Vec<u8>, Error> {
    let Layout {
        primary,
        selected,
        chunks,
        ids,
        keep_udta,
        renumbered,
    } = *layout;
    let offsets = chunk_offsets(selected, chunks, media_start);
    let mut payload = Vec::new();
    let mut tracks_written = false;

    for child in &primary.moov().children {
        match &child.kind.bytes() {
            b"mvhd" => {
                let mut bytes = held(primary, child)?.to_vec();
                if renumbered {
                    let next = selected
                        .iter()
                        .map(|slot| slot.track_id)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1);
                    let at = next_track_id_offset(&bytes);
                    patch_u32(&mut bytes, at, next);
                }
                payload.extend_from_slice(&bytes);
            },
            b"trak" => {
                if !tracks_written {
                    for (slot, item) in selected.iter().enumerate() {
                        let offsets = offsets.get(slot).map_or(&[][..], Vec::as_slice);
                        payload.extend_from_slice(&track_box(item, offsets, wide, renumbered, ids)?);
                    }
                    tracks_written = true;
                }
            },
            b"udta" => {
                if keep_udta {
                    payload.extend_from_slice(held(primary, child)?);
                }
            },
            _ => payload.extend_from_slice(held(primary, child)?),
        }
    }

    Ok(boxed(*b"moov", &payload))
}

/// The new offset of every chunk, per selected track, in the track's chunk order.
fn chunk_offsets(selected: &[Selected<'_>], chunks: &[Chunk], media_start: u64) -> Vec<Vec<u64>> {
    let mut per_track: Vec<Vec<(usize, u64)>> = vec![Vec::new(); selected.len()];
    let mut cursor = media_start;
    for chunk in chunks {
        if let Some(list) = per_track.get_mut(chunk.slot) {
            list.push((chunk.first, cursor));
        }
        cursor = cursor.saturating_add(chunk.bytes);
    }
    per_track
        .into_iter()
        .map(|mut list| {
            list.sort_by_key(|(first, _)| *first);
            list.into_iter().map(|(_, offset)| offset).collect()
        })
        .collect()
}

/// A kept track: its box with every child copied, except the chunk offset table, which is rebuilt, the
/// track header's identifier, which is patched when tracks were renumbered, and the track references,
/// which are rewritten to the output's identifiers.
fn track_box(
    item: &Selected<'_>,
    offsets: &[u64],
    wide: bool,
    renumbered: bool,
    ids: &BTreeMap<(usize, u32), u32>,
) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::new();
    for child in &item.track.range.children {
        match &child.kind.bytes() {
            b"tkhd" => {
                let mut bytes = held(item.container, child)?.to_vec();
                if renumbered {
                    let at = track_id_offset(&bytes);
                    patch_u32(&mut bytes, at, item.track_id);
                }
                payload.extend_from_slice(&bytes);
            },
            b"tref" => {
                if let (Some(bytes), _) = references_box(item, child, ids)? {
                    payload.extend_from_slice(&bytes);
                }
            },
            b"mdia" => payload.extend_from_slice(&rebuilt(item.container, child, offsets, wide)?),
            _ => payload.extend_from_slice(held(item.container, child)?),
        }
    }
    Ok(boxed(*b"trak", &payload))
}

/// A container box copied child by child, descending through the media and media information boxes to
/// the sample table, whose chunk offset table is rebuilt; everything else is copied as bytes.
fn rebuilt(container: &Container, range: &BoxRange, offsets: &[u64], wide: bool) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::new();
    for child in &range.children {
        match &child.kind.bytes() {
            b"stbl" => payload.extend_from_slice(&sample_table_box(container, child, offsets, wide)?),
            b"minf" => payload.extend_from_slice(&rebuilt(container, child, offsets, wide)?),
            _ => payload.extend_from_slice(held(container, child)?),
        }
    }
    Ok(boxed(range.kind.bytes(), &payload))
}

/// The sample table box with every child copied except the chunk offset table, which is written afresh
/// in the width the output needs, where the old one stood.
fn sample_table_box(
    container: &Container,
    stbl: &BoxRange,
    offsets: &[u64],
    wide: bool,
) -> Result<Vec<u8>, Error> {
    let mut payload = Vec::new();
    for child in &stbl.children {
        match &child.kind.bytes() {
            b"stco" | b"co64" => payload.extend_from_slice(&offset_table(offsets, wide)),
            _ => payload.extend_from_slice(held(container, child)?),
        }
    }
    Ok(boxed(*b"stbl", &payload))
}

/// A chunk offset table, narrow or wide.
fn offset_table(offsets: &[u64], wide: bool) -> Vec<u8> {
    let mut payload = Vec::with_capacity(8 + offsets.len() * 8);
    payload.extend_from_slice(&[0, 0, 0, 0]);
    payload.extend_from_slice(&u32::try_from(offsets.len()).unwrap_or(u32::MAX).to_be_bytes());
    for offset in offsets {
        if wide {
            payload.extend_from_slice(&offset.to_be_bytes());
        } else {
            payload.extend_from_slice(&u32::try_from(*offset).unwrap_or(u32::MAX).to_be_bytes());
        }
    }
    boxed(if wide { *b"co64" } else { *b"stco" }, &payload)
}

/// Overwrites four bytes at `at` with a big-endian number, when the bytes exist.
fn patch_u32(bytes: &mut [u8], at: usize, value: u32) {
    if let Some(slot) = bytes.get_mut(at..at.saturating_add(4)) {
        slot.copy_from_slice(&value.to_be_bytes());
    }
}

/// Where the track identifier sits in a track header box: after the eight-byte header, the version and
/// flags, and the two times, whose width the version decides.
fn track_id_offset(tkhd: &[u8]) -> usize {
    const VERSION_ZERO: usize = 20;
    const VERSION_ONE: usize = 28;
    if tkhd.get(8).copied() == Some(1) {
        VERSION_ONE
    } else {
        VERSION_ZERO
    }
}

/// Where the next track identifier sits in a movie header box: after the eight-byte header, the version
/// and flags, the two times, the timescale, the duration, the rate, the volume, ten reserved bytes, the
/// matrix and twenty-four predefined bytes.
fn next_track_id_offset(mvhd: &[u8]) -> usize {
    const VERSION_ZERO: usize = 104;
    const VERSION_ONE: usize = 116;
    if mvhd.get(8).copied() == Some(1) {
        VERSION_ONE
    } else {
        VERSION_ZERO
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use super::{
        Chunk, NARROW_LIMIT, earlier, fits_narrow_offsets, needs_large_header, next_track_id_offset,
        track_id_offset,
    };

    fn chunk(slot: usize, time: u64, timescale: u32) -> Chunk {
        Chunk {
            slot,
            first: 0,
            count: 1,
            bytes: 1,
            time,
            timescale,
        }
    }

    #[test]
    fn chunks_are_ordered_by_their_decode_time_as_fractions_and_then_by_track() {
        // Every case is one where a sum, a quotient or a rounded comparison would give another order.
        let cases = [
            (chunk(0, 2, 1), chunk(1, 3, 2), Ordering::Greater),
            (chunk(0, 3, 1), chunk(1, 7, 3), Ordering::Greater),
            (chunk(0, 0, 5), chunk(1, 1, 10), Ordering::Less),
            (chunk(0, 1, 2), chunk(1, 2, 4), Ordering::Less),
            (chunk(1, 1, 2), chunk(0, 2, 4), Ordering::Greater),
            (chunk(0, 100, 1000), chunk(1, 1, 2), Ordering::Less),
        ];

        for (a, b, expected) in cases {
            assert_eq!(earlier(&a, &b), expected, "{a:?} against {b:?}");
            assert_eq!(earlier(&b, &a), expected.reverse(), "{b:?} against {a:?}");
        }
    }

    #[test]
    fn a_zero_timescale_does_not_stop_the_ordering() {
        assert_eq!(earlier(&chunk(0, 1, 0), &chunk(1, 2, 0)), Ordering::Less);
    }

    #[test]
    fn the_long_header_starts_exactly_where_the_short_one_stops() {
        assert!(!needs_large_header(NARROW_LIMIT - 8));
        assert!(needs_large_header(NARROW_LIMIT - 7));
        assert!(needs_large_header(u64::MAX));
        assert!(!needs_large_header(0));
    }

    #[test]
    fn narrow_offsets_reach_the_last_addressable_byte_and_not_one_further() {
        assert!(fits_narrow_offsets(NARROW_LIMIT));
        assert!(!fits_narrow_offsets(NARROW_LIMIT + 1));
        assert!(fits_narrow_offsets(0));
    }

    #[test]
    fn the_identifier_offsets_follow_the_header_version() {
        let mut version_zero = vec![0u8; 120];
        let mut version_one = vec![0u8; 120];
        if let Some(byte) = version_one.get_mut(8) {
            *byte = 1;
        }
        if let Some(byte) = version_zero.get_mut(8) {
            *byte = 0;
        }

        assert_eq!(track_id_offset(&version_zero), 20);
        assert_eq!(track_id_offset(&version_one), 28);
        assert_eq!(next_track_id_offset(&version_zero), 104);
        assert_eq!(next_track_id_offset(&version_one), 116);
        assert_eq!(track_id_offset(&[]), 20);
        assert_eq!(next_track_id_offset(&[]), 104);
    }
}
