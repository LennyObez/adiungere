//! The reader, written without I/O so that one algorithm serves every source.
//!
//! [`Parser`] is a state machine. It hands out a [`Request`] for a byte range, is fed the bytes, and
//! either asks for the next range or finishes with a [`Container`]. A driver is a loop of a few lines, and
//! two are provided: [`parse`] for a [`Source`] and [`parse_async`] for an [`AsyncSource`]. Because the
//! requests are the same whoever drives, what a browser reads and what a command line reads are the same
//! bytes in the same order.
//!
//! The reader is frugal by construction. It reads a box header, and then reads the box in full only when
//! the box is the file type box, the movie box, or a small unknown box at the top level whose bytes are
//! worth holding so they can be digested. The media data box is stepped over by its declared size and
//! never read.

use crate::error::Error;
use crate::fourcc::FourCc;
use crate::source::{AsyncSource, Source};
use crate::tree::BoxRange;
use crate::views::{Ftyp, Mvhd, Track};

/// The most a movie box may declare and still be read in full.
pub const MOVIE_BOX_CAP: u64 = 64 * 1024 * 1024;
/// The most a file type box may declare and still be read in full.
pub const FILE_TYPE_BOX_CAP: u64 = 64 * 1024;
/// The most an unknown top-level box may declare and still be held for digesting.
pub const UNKNOWN_BOX_HOLD_CAP: u64 = 1024 * 1024;
/// How deeply boxes may nest.
pub const MAX_DEPTH: usize = 12;
/// How many children one container may enumerate.
pub const MAX_CHILDREN: usize = 1 << 16;
/// How many boxes the top level may hold.
pub const MAX_TOP_LEVEL: usize = 1 << 12;

/// Boxes whose payload is never read: media, padding and fragment structures.
const STEPPED_OVER: [&[u8; 4]; 7] = [b"mdat", b"free", b"skip", b"wide", b"moof", b"mfra", b"sidx"];

/// Containers whose children are enumerated and, in turn, descended into.
const DESCENDED: [&[u8; 4]; 9] = [
    b"moov", b"trak", b"edts", b"mdia", b"minf", b"dinf", b"stbl", b"mvex", b"tref",
];

/// A range of bytes the parser needs next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// The first byte.
    pub offset: u64,
    /// How many bytes, exactly.
    pub length: usize,
}

/// What the parser wants a driver to do next.
#[derive(Debug)]
pub enum Step {
    /// Read this range and feed it back.
    Read(Request),
    /// Nothing more is needed.
    Done(Container),
}

#[derive(Debug, Clone)]
struct Held {
    offset: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy)]
enum Outstanding {
    Header {
        offset: u64,
    },
    Body {
        kind: FourCc,
        offset: u64,
        size: u64,
        header: u8,
    },
}

/// The sans-I/O reader. See the module documentation for how it is driven.
#[derive(Debug)]
pub struct Parser {
    length: u64,
    position: u64,
    top_level: Vec<BoxRange>,
    held: Vec<Held>,
    outstanding: Option<Outstanding>,
    finished: bool,
}

impl Parser {
    /// Starts reading a source of the given length.
    #[must_use]
    pub const fn new(length: u64) -> Self {
        Self {
            length,
            position: 0,
            top_level: Vec::new(),
            held: Vec::new(),
            outstanding: None,
            finished: false,
        }
    }

    /// Asks what to do next.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Protocol`] when a request is outstanding, and the reading errors when the file's
    /// structure is inconsistent.
    pub fn step(&mut self) -> Result<Step, Error> {
        if self.outstanding.is_some() || self.finished {
            return Err(Error::Protocol);
        }

        if self.position >= self.length {
            return self.finish();
        }

        if self.top_level.len() >= MAX_TOP_LEVEL {
            return Err(Error::TooMany {
                kind: FourCc(*b"    "),
                offset: self.position,
            });
        }

        let remaining = self.length.saturating_sub(self.position);
        let length = usize::try_from(remaining.min(16)).unwrap_or(16);
        self.outstanding = Some(Outstanding::Header {
            offset: self.position,
        });

        Ok(Step::Read(Request {
            offset: self.position,
            length,
        }))
    }

    /// Hands the parser the bytes it asked for.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Protocol`] when nothing was requested, and the reading errors when the bytes
    /// describe an inconsistent structure.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let Some(outstanding) = self.outstanding.take() else {
            return Err(Error::Protocol);
        };

        match outstanding {
            Outstanding::Header { offset } => self.on_header(offset, bytes),
            Outstanding::Body {
                kind,
                offset,
                size,
                header,
            } => self.on_body(kind, offset, size, header, bytes),
        }
    }

    /// Records everything from `offset` to the end of the file as a clipped tail that is not a box.
    fn push_tail(&mut self, offset: u64, bytes: &[u8]) {
        let mut kind = [0; 4];
        for (slot, byte) in kind.iter_mut().zip(bytes) {
            *slot = *byte;
        }
        self.top_level.push(BoxRange {
            kind: FourCc(kind),
            offset,
            size: self.length.saturating_sub(offset),
            header: 0,
            fields: 0,
            children: Vec::new(),
            clipped: true,
        });
        self.position = self.length;
    }

    fn on_header(&mut self, offset: u64, bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() < 8 {
            // Fewer than eight bytes remain: a tail no header fits in. Kept as a range so that the ranges
            // still tile the file, and marked as clipped because it is not a box.
            self.push_tail(offset, bytes);
            return Ok(());
        }

        if offset == 0 && !looks_like_a_box_start(bytes) {
            return Err(Error::NotIsobmff);
        }

        // Once the movie box has been read, whatever follows the last well-formed box is tolerated as a
        // clipped tail: a recorder that pre-allocates its file leaves the unused space after the movie
        // box, and a recorder that lost power leaves a media data box that declares more than the file
        // holds. Both files play, both are worth describing, and the tail is a range like any other so
        // the ranges still tile the file. Before the movie box nothing can be tolerated, because without
        // it there is nothing to describe.
        let moov_seen = self.top_level.iter().any(|range| range.kind == b"moov");
        let header = match read_header(bytes, offset, self.length) {
            // A box type is four characters; pre-allocated space is zeros, and zeros read as a box of
            // type NUL NUL NUL NUL that extends to the end of the file.
            Ok((kind, _, _)) if moov_seen && kind.bytes().contains(&0) => None,
            Ok(parsed) => Some(parsed),
            Err(_) if moov_seen => None,
            Err(error) => return Err(error),
        };
        let Some((kind, size, header)) = header else {
            self.push_tail(offset, bytes);
            return Ok(());
        };

        let end = offset.checked_add(size).filter(|end| *end <= self.length);
        let Some(end) = end else {
            if moov_seen {
                self.top_level.push(BoxRange {
                    kind,
                    offset,
                    size: self.length.saturating_sub(offset),
                    header,
                    fields: 0,
                    children: Vec::new(),
                    clipped: true,
                });
                self.position = self.length;
                return Ok(());
            }
            return Err(Error::Overrun {
                kind,
                offset,
                size,
                limit: self.length,
            });
        };

        let fetch_whole = if kind == b"moov" {
            if self.top_level.iter().any(|range| range.kind == b"moov") {
                return Err(Error::TwoMovieBoxes { offset });
            }
            Some(MOVIE_BOX_CAP)
        } else if kind == b"ftyp" {
            Some(FILE_TYPE_BOX_CAP)
        } else if STEPPED_OVER.contains(&&kind.bytes()) {
            None
        } else if size <= UNKNOWN_BOX_HOLD_CAP {
            Some(UNKNOWN_BOX_HOLD_CAP)
        } else {
            None
        };

        if let Some(cap) = fetch_whole {
            if size > cap {
                return Err(Error::Oversized {
                    kind,
                    offset,
                    size,
                    cap,
                });
            }
            self.outstanding = Some(Outstanding::Body {
                kind,
                offset,
                size,
                header,
            });
            return Ok(());
        }

        self.top_level.push(BoxRange {
            kind,
            offset,
            size,
            header,
            fields: 0,
            children: Vec::new(),
            clipped: false,
        });
        self.position = end;
        Ok(())
    }

    fn on_body(
        &mut self,
        kind: FourCc,
        offset: u64,
        size: u64,
        header: u8,
        bytes: &[u8],
    ) -> Result<(), Error> {
        if bytes.len() as u64 != size {
            return Err(Error::Protocol);
        }

        let children = if kind == b"moov" {
            let payload = bytes.get(usize::from(header)..).unwrap_or(&[]);
            enumerate(
                payload,
                offset.saturating_add(u64::from(header)),
                kind,
                offset,
                1,
                Policy::Strict,
            )?
        } else {
            Vec::new()
        };

        self.held.push(Held {
            offset,
            bytes: bytes.to_vec(),
        });
        self.top_level.push(BoxRange {
            kind,
            offset,
            size,
            header,
            fields: 0,
            children,
            clipped: false,
        });
        self.position = offset.saturating_add(size);
        Ok(())
    }

    /// The parser wants the next request only after the previous body was fed, which `feed` guarantees;
    /// so the body request is issued from here rather than from `step` directly.
    fn pending_body(&self) -> Option<Request> {
        match self.outstanding {
            Some(Outstanding::Body { offset, size, .. }) => Some(Request {
                offset,
                length: usize::try_from(size).ok()?,
            }),
            _ => None,
        }
    }

    fn finish(&mut self) -> Result<Step, Error> {
        if !self.top_level.iter().any(|range| range.kind == b"moov") {
            return Err(Error::NoMovieBox);
        }

        self.finished = true;

        Ok(Step::Done(Container {
            length: self.length,
            top_level: std::mem::take(&mut self.top_level),
            held: std::mem::take(&mut self.held),
        }))
    }
}

/// Whether the first bytes could open a box: a plausible size and a printable type.
fn looks_like_a_box_start(bytes: &[u8]) -> bool {
    /// The types a base media file can begin with. Arbitrary text has a one in a few billion chance of
    /// spelling one of these after a plausible size, which is the point of checking.
    const OPENING: [&[u8; 4]; 11] = [
        b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide", b"uuid", b"moof", b"sidx", b"styp", b"pdin",
    ];
    let kind = bytes.get(4..8).unwrap_or(&[]);
    let size = be_u32(bytes, 0).unwrap_or(0);

    OPENING.iter().any(|opening| *opening == kind) && (size == 1 || size >= 8 || size == 0)
}

fn be_u32(bytes: &[u8], at: usize) -> Option<u32> {
    let slice = bytes.get(at..at.checked_add(4)?)?;
    let array: [u8; 4] = slice.try_into().ok()?;
    Some(u32::from_be_bytes(array))
}

fn be_u64(bytes: &[u8], at: usize) -> Option<u64> {
    let slice = bytes.get(at..at.checked_add(8)?)?;
    let array: [u8; 8] = slice.try_into().ok()?;
    Some(u64::from_be_bytes(array))
}

/// Reads a box header. `limit` is the end of the enclosing region, used for a size of zero.
fn read_header(bytes: &[u8], offset: u64, limit: u64) -> Result<(FourCc, u64, u8), Error> {
    let kind = bytes
        .get(4..8)
        .and_then(|slice| <[u8; 4]>::try_from(slice).ok())
        .map(FourCc)
        .ok_or(Error::NotIsobmff)?;
    let size32 = be_u32(bytes, 0).ok_or(Error::NotIsobmff)?;

    let (size, header) = match size32 {
        0 => (limit.saturating_sub(offset), 8),
        1 => {
            let large = be_u64(bytes, 8).ok_or(Error::Truncated {
                kind,
                offset,
                field: "64-bit size",
            })?;
            (large, 16)
        },
        other => (u64::from(other), 8),
    };

    if size < u64::from(header) {
        return Err(Error::Undersized { kind, offset, size });
    }

    Ok((kind, size, header))
}

/// How a container's children are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Policy {
    /// A child that overruns its parent, or bytes that do not read as a box, are errors. The standard
    /// boxes of a movie are consistent or the file is not readable.
    Strict,
    /// A child that overruns is clipped to the parent and flagged; bytes that do not read as a box become
    /// a clipped tail. A vendor writes what it likes under the user-data box and inside a sample entry,
    /// and the ranges still have to tile the parent.
    Lenient,
}

/// Enumerates the boxes in a payload, descending into the containers the reader knows.
fn enumerate(
    payload: &[u8],
    base: u64,
    parent: FourCc,
    parent_offset: u64,
    depth: usize,
    policy: Policy,
) -> Result<Vec<BoxRange>, Error> {
    if depth > MAX_DEPTH {
        return Err(Error::TooDeep { offset: base });
    }

    let lenient = policy == Policy::Lenient;
    let limit = base.saturating_add(payload.len() as u64);
    let mut children = Vec::new();
    let mut cursor = 0usize;

    while cursor < payload.len() {
        if children.len() >= MAX_CHILDREN {
            return Err(Error::TooMany {
                kind: parent,
                offset: parent_offset,
            });
        }

        let offset = base.saturating_add(cursor as u64);
        let remaining = payload.get(cursor..).unwrap_or(&[]);

        if remaining.len() < 8 {
            if lenient {
                children.push(tail_range(remaining, offset));
                break;
            }
            return Err(Error::Truncated {
                kind: parent,
                offset: parent_offset,
                field: "child box header",
            });
        }

        let (kind, declared, header) = match read_header(remaining, offset, limit) {
            Ok(parsed) => parsed,
            Err(_) if lenient => {
                children.push(tail_range(remaining, offset));
                break;
            },
            Err(error) => return Err(error),
        };

        let available = remaining.len() as u64;
        let (size, clipped) = if declared > available {
            if !lenient {
                return Err(Error::Overrun {
                    kind,
                    offset,
                    size: declared,
                    limit,
                });
            }
            (available, true)
        } else {
            (declared, false)
        };

        // `size` is at most the slice length here, so it fits a `usize`.
        let size_usize = usize::try_from(size).unwrap_or(remaining.len());
        let child = Child {
            kind,
            offset,
            header,
            bytes: remaining.get(..size_usize).unwrap_or(remaining),
            parent,
            depth: depth.saturating_add(1),
        };
        let (fields, grandchildren) = if clipped {
            (0, Vec::new())
        } else {
            child.enumerate()?
        };

        children.push(BoxRange {
            kind,
            offset,
            size,
            header,
            fields,
            children: grandchildren,
            clipped,
        });

        cursor = cursor.saturating_add(size_usize);
    }

    Ok(children)
}

/// The version, flags and entry count that open a sample description box.
const SAMPLE_DESCRIPTION_FIELDS: usize = 8;

/// One child about to be enumerated: its whole bytes, header included, and where it sits.
struct Child<'a> {
    kind: FourCc,
    offset: u64,
    header: u8,
    bytes: &'a [u8],
    parent: FourCc,
    depth: usize,
}

impl Child<'_> {
    /// Decides how many of the child's own fields precede its children, and enumerates those children.
    fn enumerate(&self) -> Result<(u32, Vec<BoxRange>), Error> {
        let body = self.bytes.get(usize::from(self.header)..).unwrap_or(&[]);
        let body_offset = self.offset.saturating_add(u64::from(self.header));

        if DESCENDED.contains(&&self.kind.bytes()) {
            let children = enumerate(
                body,
                body_offset,
                self.kind,
                self.offset,
                self.depth,
                Policy::Strict,
            )?;
            return Ok((0, children));
        }

        if self.kind == b"udta" {
            let children = enumerate(
                body,
                body_offset,
                self.kind,
                self.offset,
                self.depth,
                Policy::Lenient,
            )?;
            return Ok((0, children));
        }

        if self.kind == b"stsd" {
            // Version, flags and the entry count precede the entries. The count is advisory: entries are
            // enumerated by size, and a count that disagrees is reported by the typed view.
            let fields = SAMPLE_DESCRIPTION_FIELDS.min(body.len());
            let entries = body.get(fields..).unwrap_or(&[]);
            let children = enumerate(
                entries,
                body_offset.saturating_add(fields as u64),
                self.kind,
                self.offset,
                self.depth,
                Policy::Lenient,
            )?;
            return Ok((u32::try_from(fields).unwrap_or(0), children));
        }

        if self.parent == b"stsd" {
            // A sample entry: its fixed fields, then the boxes that describe the decoder. An entry whose
            // layout this reader does not know is a range with no children, which loses nothing.
            if let Some(fixed) = fixed_fields_length(self.kind, self.bytes)
                && fixed <= self.bytes.len()
            {
                let inner = self.bytes.get(fixed..).unwrap_or(&[]);
                let fields = fixed.saturating_sub(usize::from(self.header));
                let children = enumerate(
                    inner,
                    self.offset.saturating_add(fixed as u64),
                    self.kind,
                    self.offset,
                    self.depth,
                    Policy::Lenient,
                )?;
                return Ok((u32::try_from(fields).unwrap_or(0), children));
            }
        }

        Ok((0, Vec::new()))
    }
}

/// A run of bytes that is not a box, kept as a clipped range so the parent still tiles.
fn tail_range(bytes: &[u8], offset: u64) -> BoxRange {
    let mut kind = [0; 4];
    for (slot, byte) in kind.iter_mut().zip(bytes) {
        *slot = *byte;
    }
    BoxRange {
        kind: FourCc(kind),
        offset,
        size: bytes.len() as u64,
        header: 0,
        fields: 0,
        children: Vec::new(),
        clipped: true,
    }
}

/// How many bytes of fixed fields, header included, precede the child boxes of a sample entry, or `None`
/// when the entry's layout is not one this reader knows.
fn fixed_fields_length(kind: FourCc, entry: &[u8]) -> Option<usize> {
    const VISUAL: [&[u8; 4]; 6] = [b"avc1", b"avc3", b"hvc1", b"hev1", b"mp4v", b"encv"];
    const AUDIO: [&[u8; 4]; 4] = [b"mp4a", b"enca", b"ac-3", b"ec-3"];

    if VISUAL.contains(&&kind.bytes()) {
        // Header, six reserved bytes, the data reference index, sixteen predefined and reserved bytes,
        // width, height, two resolutions, four reserved bytes, a frame count, a thirty-two byte compressor
        // name, a depth and a predefined value.
        return Some(8 + 6 + 2 + 16 + 2 + 2 + 4 + 4 + 4 + 2 + 32 + 2 + 2);
    }

    if AUDIO.contains(&&kind.bytes()) {
        // The QuickTime version field decides how many fields precede the children.
        let version = entry.get(16..18).and_then(|slice| {
            let array: [u8; 2] = slice.try_into().ok()?;
            Some(u16::from_be_bytes(array))
        })?;
        return match version {
            0 => Some(8 + 6 + 2 + 8 + 2 + 2 + 2 + 2 + 4),
            1 => Some(8 + 6 + 2 + 8 + 2 + 2 + 2 + 2 + 4 + 16),
            2 => Some(8 + 6 + 2 + 8 + 2 + 2 + 2 + 2 + 4 + 36),
            _ => None,
        };
    }

    None
}

/// A file, read: its top-level boxes, its movie box enumerated, and the bytes of the boxes worth holding.
#[derive(Debug, Clone)]
pub struct Container {
    length: u64,
    top_level: Vec<BoxRange>,
    held: Vec<Held>,
}

impl Container {
    /// The length of the file.
    #[must_use]
    pub const fn length(&self) -> u64 {
        self.length
    }

    /// Every top-level box in file order, the movie box with its tree.
    #[must_use]
    pub fn top_level(&self) -> &[BoxRange] {
        &self.top_level
    }

    /// The movie box. The parser refuses to finish without one, so this cannot fail.
    #[must_use]
    pub fn moov(&self) -> &BoxRange {
        // The parser guarantees exactly one movie box; the fallback can never be reached and exists so that
        // the method has no failure path.
        self.top_level
            .iter()
            .find(|range| range.kind == b"moov")
            .unwrap_or_else(|| self.top_level.first().map_or(&EMPTY, |first| first))
    }

    /// The bytes of a box, header included, if the reader held them.
    #[must_use]
    pub fn bytes_of(&self, range: &BoxRange) -> Option<&[u8]> {
        let end = range.offset.checked_add(range.size)?;
        self.held.iter().find_map(|held| {
            let held_end = held.offset.checked_add(held.bytes.len() as u64)?;
            if range.offset < held.offset || end > held_end {
                return None;
            }
            let start = usize::try_from(range.offset.checked_sub(held.offset)?).ok()?;
            let stop = usize::try_from(end.checked_sub(held.offset)?).ok()?;
            held.bytes.get(start..stop)
        })
    }

    /// The payload of a box, if the reader held it.
    #[must_use]
    pub fn payload_of(&self, range: &BoxRange) -> Option<&[u8]> {
        self.bytes_of(range)?.get(usize::from(range.header)..)
    }

    /// The file type box, if the file has one.
    #[must_use]
    pub fn ftyp(&self) -> Option<Ftyp> {
        let range = self.top_level.iter().find(|range| range.kind == b"ftyp")?;
        Ftyp::read(self.payload_of(range)?)
    }

    /// The movie header.
    ///
    /// # Errors
    ///
    /// Returns an error when the movie box has no header or the header is malformed.
    pub fn mvhd(&self) -> Result<Mvhd, Error> {
        let moov = self.moov();
        let range = moov.child(b"mvhd").ok_or(Error::Truncated {
            kind: FourCc(*b"moov"),
            offset: moov.offset,
            field: "movie header",
        })?;
        Mvhd::read(self.payload_of(range).unwrap_or(&[]), range.offset)
    }

    /// Every track, in file order, with its sample tables decoded.
    ///
    /// # Errors
    ///
    /// Returns an error when a track's boxes are missing or contradict each other.
    pub fn tracks(&self) -> Result<Vec<Track>, Error> {
        self.moov()
            .children_of(b"trak")
            .enumerate()
            .map(|(index, trak)| Track::read(self, trak, index))
            .collect()
    }

    /// The children of the user-data box under the movie box, as ranges, or nothing when there is none.
    #[must_use]
    pub fn udta_children(&self) -> Vec<&BoxRange> {
        self.moov()
            .child(b"udta")
            .map(|udta| udta.children.iter().collect())
            .unwrap_or_default()
    }

    /// The top-level boxes that are neither the file type, the movie, media data nor padding.
    #[must_use]
    pub fn unknown_top_level(&self) -> Vec<&BoxRange> {
        const KNOWN: [&[u8; 4]; 9] = [
            b"ftyp", b"moov", b"mdat", b"free", b"skip", b"wide", b"moof", b"mfra", b"sidx",
        ];
        // A clipped tail is not a box, so it is not an unknown one either.
        self.top_level
            .iter()
            .filter(|range| !KNOWN.contains(&&range.kind.bytes()) && !range.clipped)
            .collect()
    }

    /// The clipped range at the end of the file, when the file ends with bytes that are not a well-formed
    /// box or with a box that declares more than the file holds.
    #[must_use]
    pub fn clipped_tail(&self) -> Option<&BoxRange> {
        self.top_level.last().filter(|range| range.clipped)
    }

    /// Whether the movie box precedes the first media data box. `None` when there is no media data box.
    #[must_use]
    pub fn moov_before_mdat(&self) -> Option<bool> {
        let moov = self.top_level.iter().position(|range| range.kind == b"moov")?;
        let mdat = self.top_level.iter().position(|range| range.kind == b"mdat")?;
        Some(moov < mdat)
    }

    /// The media data boxes, as ranges.
    #[must_use]
    pub fn mdat(&self) -> Vec<&BoxRange> {
        self.top_level
            .iter()
            .filter(|range| range.kind == b"mdat")
            .collect()
    }

    /// Every box with the path that leads to it, depth first, in file order.
    #[must_use]
    pub fn walk(&self) -> Vec<(String, &BoxRange)> {
        let mut visited = Vec::new();
        for range in &self.top_level {
            range.walk("", &mut visited);
        }
        visited
    }

    /// Whether the top-level ranges tile the file and every enumerated container tiles its payload.
    #[must_use]
    pub fn ranges_tile_the_file(&self) -> bool {
        let mut cursor = 0;
        for range in &self.top_level {
            if range.offset != cursor || !range.children_tile_payload() {
                return false;
            }
            cursor = range.end();
        }
        cursor == self.length
    }
}

static EMPTY: BoxRange = BoxRange {
    kind: FourCc(*b"    "),
    offset: 0,
    size: 0,
    header: 0,
    fields: 0,
    children: Vec::new(),
    clipped: true,
};

/// Reads a whole file's structure from a synchronous source.
///
/// # Errors
///
/// Returns the first structural problem, or the source's failure.
pub fn parse<S: Source>(source: &mut S) -> Result<Container, Error> {
    let length = source.length()?;
    let mut parser = Parser::new(length);

    loop {
        match parser.step()? {
            Step::Done(container) => return Ok(container),
            Step::Read(request) => {
                let bytes = source.read_range(request.offset, request.length)?;
                parser.feed(&bytes)?;

                if let Some(body) = parser.pending_body() {
                    let bytes = source.read_range(body.offset, body.length)?;
                    parser.feed(&bytes)?;
                }
            },
        }
    }
}

/// Reads a whole file's structure from an asynchronous source, with the same requests in the same order
/// as [`parse`].
///
/// # Errors
///
/// Returns the first structural problem, or the source's failure.
pub async fn parse_async<S: AsyncSource>(source: &mut S) -> Result<Container, Error> {
    let length = source.length().await?;
    let mut parser = Parser::new(length);

    loop {
        match parser.step()? {
            Step::Done(container) => return Ok(container),
            Step::Read(request) => {
                let bytes = source.read_range(request.offset, request.length).await?;
                parser.feed(&bytes)?;

                if let Some(body) = parser.pending_body() {
                    let bytes = source.read_range(body.offset, body.length).await?;
                    parser.feed(&bytes)?;
                }
            },
        }
    }
}

impl Parser {
    /// The body request that a header just fed calls for, if any. Exposed so that a driver written outside
    /// this crate can follow the same protocol as the two drivers here.
    #[must_use]
    pub fn body_request(&self) -> Option<Request> {
        self.pending_body()
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Container, FILE_TYPE_BOX_CAP, Held, MAX_CHILDREN, MAX_DEPTH, MAX_TOP_LEVEL, MOVIE_BOX_CAP, Parser,
        Policy, Step, UNKNOWN_BOX_HOLD_CAP, enumerate, fixed_fields_length, looks_like_a_box_start,
        read_header,
    };
    use crate::error::Error;
    use crate::fourcc::FourCc;
    use crate::tree::BoxRange;

    #[test]
    fn the_caps_are_the_documented_sizes() {
        // Act and assert
        assert_eq!(MOVIE_BOX_CAP, 67_108_864);
        assert_eq!(FILE_TYPE_BOX_CAP, 65_536);
        assert_eq!(UNKNOWN_BOX_HOLD_CAP, 1_048_576);
        assert_eq!(MAX_DEPTH, 12);
        assert_eq!(MAX_CHILDREN, 65_536);
        assert_eq!(MAX_TOP_LEVEL, 4096);
    }

    #[test]
    fn the_fixed_fields_of_each_entry_layout_are_the_standard_lengths() {
        // Act and assert
        assert_eq!(fixed_fields_length(FourCc(*b"avc1"), &[0; 90]), Some(86));
        assert_eq!(fixed_fields_length(FourCc(*b"hvc1"), &[0; 90]), Some(86));
        let mut audio = [0u8; 80];
        assert_eq!(fixed_fields_length(FourCc(*b"mp4a"), &audio), Some(36));
        audio[17] = 1;
        assert_eq!(fixed_fields_length(FourCc(*b"mp4a"), &audio), Some(52));
        audio[17] = 2;
        assert_eq!(fixed_fields_length(FourCc(*b"mp4a"), &audio), Some(72));
        audio[17] = 3;
        assert_eq!(fixed_fields_length(FourCc(*b"mp4a"), &audio), None);
        assert_eq!(fixed_fields_length(FourCc(*b"mp4a"), &[0; 10]), None);
        assert_eq!(fixed_fields_length(FourCc(*b"zzzz"), &[0; 90]), None);
    }

    #[test]
    fn a_box_start_needs_a_known_opening_type_and_a_plausible_size() {
        // Act and assert
        assert!(looks_like_a_box_start(b"\0\0\0\x18ftypmp42"));
        assert!(looks_like_a_box_start(b"\0\0\0\x01moov\0\0\0\0"));
        assert!(looks_like_a_box_start(b"\0\0\0\0mdat"));
        assert!(!looks_like_a_box_start(b"\0\0\0\x07ftyp"));
        assert!(!looks_like_a_box_start(b"\0\0\0\x18zzzz"));
        assert!(!looks_like_a_box_start(b"not a movie"));
    }

    #[test]
    fn a_header_resolves_each_size_form() {
        // Act
        let short = read_header(b"\0\0\0\x10ftyp", 0, 100).unwrap();
        let large = read_header(b"\0\0\0\x01mdat\0\0\0\0\0\0\x01\x00", 10, 1000).unwrap();
        let to_end = read_header(b"\0\0\0\0mdat", 40, 100).unwrap();
        let undersized = read_header(b"\0\0\0\x04free", 0, 100);
        let short_large = read_header(b"\0\0\0\x01mdat\0\0", 0, 100);

        // Assert
        assert_eq!(short, (FourCc(*b"ftyp"), 16, 8));
        assert_eq!(large, (FourCc(*b"mdat"), 256, 16));
        assert_eq!(to_end, (FourCc(*b"mdat"), 60, 8));
        assert!(matches!(undersized, Err(Error::Undersized { size: 4, .. })));
        assert!(matches!(short_large, Err(Error::Truncated { .. })));
    }

    #[test]
    fn a_strict_container_refuses_an_overrun_and_a_lenient_one_clips_it() {
        // Arrange
        let payload = b"\0\0\0\x20zzzz1234";

        // Act
        let strict = enumerate(payload, 8, FourCc(*b"stbl"), 0, 1, Policy::Strict);
        let lenient = enumerate(payload, 8, FourCc(*b"udta"), 0, 1, Policy::Lenient).unwrap();

        // Assert
        assert!(matches!(strict, Err(Error::Overrun { size: 32, .. })));
        assert_eq!(lenient.len(), 1);
        assert!(lenient.first().unwrap().clipped);
        assert_eq!(lenient.first().unwrap().size, 12);
    }

    #[test]
    fn a_strict_container_refuses_a_tail_that_is_not_a_box_and_a_lenient_one_keeps_it() {
        // Arrange
        let payload = b"\0\0\0\x0czzzz1234abc";

        // Act
        let strict = enumerate(payload, 8, FourCc(*b"stbl"), 0, 1, Policy::Strict);
        let lenient = enumerate(payload, 8, FourCc(*b"udta"), 0, 1, Policy::Lenient).unwrap();

        // Assert
        assert!(matches!(strict, Err(Error::Truncated { .. })));
        assert_eq!(lenient.len(), 2);
        assert!(lenient.get(1).unwrap().clipped);
        assert_eq!(lenient.get(1).unwrap().size, 3);
        assert_eq!(lenient.get(1).unwrap().offset, 20);
    }

    #[test]
    fn nesting_past_the_cap_is_refused() {
        // Arrange: a movie box wrapping itself thirteen times.
        let mut payload: Vec<u8> = Vec::new();
        for _ in 0..(MAX_DEPTH + 2) {
            let inner = payload.clone();
            payload = Vec::new();
            payload.extend_from_slice(&u32::try_from(inner.len() + 8).unwrap().to_be_bytes());
            payload.extend_from_slice(b"moov");
            payload.extend_from_slice(&inner);
        }

        // Act
        let outcome = enumerate(&payload, 8, FourCc(*b"moov"), 0, 1, Policy::Strict);

        // Assert
        assert!(matches!(outcome, Err(Error::TooDeep { .. })), "got {outcome:?}");
    }

    #[test]
    fn the_parser_refuses_to_be_driven_out_of_order() {
        // Arrange
        let mut parser = Parser::new(100);

        // Act
        let fed_early = parser.feed(&[0; 16]);
        let first = parser.step();
        let second = parser.step();

        // Assert
        assert!(matches!(fed_early, Err(Error::Protocol)));
        assert!(matches!(first, Ok(Step::Read(request)) if request.offset == 0 && request.length == 16));
        assert!(matches!(second, Err(Error::Protocol)));
    }

    #[test]
    fn a_body_of_the_wrong_length_is_refused() {
        // Arrange
        let mut parser = Parser::new(100);
        let _ = parser.step();
        parser.feed(b"\0\0\0\x20ftypmp42\0\0\0\0mp42").unwrap();
        let body = parser.body_request().unwrap();

        // Act
        let outcome = parser.feed(&[0; 5]);

        // Assert
        assert_eq!(body.length, 32);
        assert!(matches!(outcome, Err(Error::Protocol)));
    }

    #[test]
    fn a_file_type_box_exactly_at_its_cap_is_read_and_one_byte_over_is_refused() {
        // Arrange
        let at_cap = u32::try_from(FILE_TYPE_BOX_CAP).unwrap();
        let mut exact = Parser::new(u64::from(at_cap) + 100);
        let _ = exact.step();
        let mut over = Parser::new(u64::from(at_cap) + 100);
        let _ = over.step();

        // Act
        let mut header = at_cap.to_be_bytes().to_vec();
        header.extend_from_slice(b"ftypmp42\0\0\0\0");
        let accepted = exact.feed(&header);
        let mut header = (at_cap + 1).to_be_bytes().to_vec();
        header.extend_from_slice(b"ftypmp42\0\0\0\0");
        let refused = over.feed(&header);

        // Assert
        assert!(accepted.is_ok());
        assert_eq!(exact.body_request().map(|body| body.length), Some(65_536));
        assert!(matches!(refused, Err(Error::Oversized { .. })));
    }

    #[test]
    fn a_container_whose_last_child_is_a_bare_header_is_read_in_full() {
        // Arrange: a movie box holding a header-only padding box as its last child.
        let payload = b"\0\0\0\x0cmvhd1234\0\0\0\x08free";

        // Act
        let children = enumerate(payload, 8, FourCc(*b"moov"), 0, 1, Policy::Strict).unwrap();

        // Assert
        assert_eq!(children.len(), 2);
        assert_eq!(children.get(1).unwrap().kind, b"free");
        assert_eq!(children.get(1).unwrap().size, 8);
        assert!(!children.get(1).unwrap().clipped);
    }

    #[test]
    fn an_undersized_child_is_an_error_when_strict_and_a_tail_when_lenient() {
        // Arrange: a child declaring four bytes, fewer than its own header.
        let payload = b"\0\0\0\x04zzzzabcdefgh";

        // Act
        let strict = enumerate(payload, 8, FourCc(*b"stbl"), 0, 1, Policy::Strict);
        let lenient = enumerate(payload, 8, FourCc(*b"udta"), 0, 1, Policy::Lenient).unwrap();

        // Assert
        assert!(matches!(strict, Err(Error::Undersized { size: 4, .. })));
        assert_eq!(lenient.len(), 1);
        assert!(lenient.first().unwrap().clipped);
        assert_eq!(lenient.first().unwrap().size, 16);
    }

    #[test]
    fn nesting_exactly_at_the_cap_is_read() {
        // Arrange: containers nested to the cap, the innermost a leaf.
        let mut payload: Vec<u8> = b"\0\0\0\x08free".to_vec();
        for _ in 0..(MAX_DEPTH - 1) {
            let inner = payload.clone();
            payload = Vec::new();
            payload.extend_from_slice(&u32::try_from(inner.len() + 8).unwrap().to_be_bytes());
            payload.extend_from_slice(b"moov");
            payload.extend_from_slice(&inner);
        }

        // Act
        let outcome = enumerate(&payload, 8, FourCc(*b"moov"), 0, 1, Policy::Strict);

        // Assert
        assert!(outcome.is_ok(), "got {outcome:?}");
    }

    #[test]
    fn a_top_level_box_past_the_cap_of_the_file_is_refused() {
        // Arrange
        let mut parser = Parser::new(20);
        let _ = parser.step();

        // Act
        let outcome = parser.feed(b"\0\0\0\x40ftypmp42\0\0\0\0mp42");

        // Assert
        assert!(matches!(
            outcome,
            Err(Error::Overrun {
                size: 64,
                limit: 20,
                ..
            })
        ));
    }

    fn range(kind: [u8; 4], offset: u64, size: u64) -> BoxRange {
        BoxRange {
            kind: FourCc(kind),
            offset,
            size,
            header: 8,
            fields: 0,
            children: Vec::new(),
            clipped: false,
        }
    }

    fn container(length: u64, top_level: Vec<BoxRange>, held: Vec<Held>) -> Container {
        Container {
            length,
            top_level,
            held,
        }
    }

    #[test]
    fn the_ranges_tile_the_file_only_without_a_gap_an_overlap_or_a_shortfall() {
        // Act and assert
        assert!(
            container(30, vec![range(*b"ftyp", 0, 10), range(*b"mdat", 10, 20)], vec![])
                .ranges_tile_the_file()
        );
        assert!(
            !container(30, vec![range(*b"ftyp", 0, 10), range(*b"mdat", 12, 18)], vec![])
                .ranges_tile_the_file()
        );
        assert!(
            !container(30, vec![range(*b"ftyp", 0, 10), range(*b"mdat", 8, 22)], vec![])
                .ranges_tile_the_file()
        );
        assert!(
            !container(31, vec![range(*b"ftyp", 0, 10), range(*b"mdat", 10, 20)], vec![])
                .ranges_tile_the_file()
        );
        let mut parent = range(*b"moov", 0, 30);
        parent.children = vec![range(*b"mvhd", 8, 10)];
        assert!(
            !container(30, vec![parent], vec![]).ranges_tile_the_file(),
            "the children have to tile too"
        );
    }

    #[test]
    fn the_bytes_of_a_range_are_served_only_from_a_hold_that_covers_it_entirely() {
        // Arrange
        let held = Held {
            offset: 100,
            bytes: (0..50u8).collect(),
        };
        let container = container(1000, Vec::new(), vec![held]);

        // Act and assert
        assert_eq!(
            container.bytes_of(&range(*b"zvnd", 100, 50)).map(<[u8]>::len),
            Some(50)
        );
        assert_eq!(
            container.bytes_of(&range(*b"zvnd", 110, 10)),
            Some(&[10, 11, 12, 13, 14, 15, 16, 17, 18, 19][..])
        );
        assert!(
            container.bytes_of(&range(*b"zvnd", 99, 10)).is_none(),
            "starts before the hold"
        );
        assert!(
            container.bytes_of(&range(*b"zvnd", 140, 20)).is_none(),
            "ends after the hold"
        );
        assert!(
            container.bytes_of(&range(*b"zvnd", 90, 80)).is_none(),
            "spans past both edges"
        );
        assert!(
            container.bytes_of(&range(*b"zvnd", u64::MAX, 2)).is_none(),
            "an end that overflows"
        );
    }

    #[test]
    fn the_movie_box_position_is_relative_to_the_first_media_data_box() {
        // Act and assert
        assert_eq!(
            container(30, vec![range(*b"moov", 0, 10), range(*b"mdat", 10, 20)], vec![]).moov_before_mdat(),
            Some(true)
        );
        assert_eq!(
            container(30, vec![range(*b"mdat", 0, 20), range(*b"moov", 20, 10)], vec![]).moov_before_mdat(),
            Some(false)
        );
        assert_eq!(
            container(10, vec![range(*b"moov", 0, 10)], vec![]).moov_before_mdat(),
            None
        );
    }
}
