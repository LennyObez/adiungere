//! The container format, read as byte ranges.
//!
//! An ISO base media file is a sequence of boxes, and most of what a dashcam writes into one is not
//! standard: a vendor box under the user-data box carrying telemetry, a vendor box at the top level carrying
//! a model code, an unknown child inside a sample entry. Every general-purpose library reads the boxes it
//! knows and drops the rest, which is exactly the material this product exists to preserve.
//!
//! So this crate does the opposite. Every box is a **range**: an offset, a size and a type, kept whether or
//! not the type is known. A typed view is decoded only where a value is needed to locate samples or to
//! describe a track, and a typed view never replaces the range it was read from. The user-data box is
//! enumerated, never interpreted. A sample entry is a range with its children enumerated as ranges, and its
//! decoder configuration is copied, never resynthesised.
//!
//! Reading is **bounded and sans-I/O**. The [`Parser`] asks for byte ranges and is handed bytes; it never
//! touches the media data box and it reads the movie box exactly once. A synchronous driver and an
//! asynchronous driver share that one algorithm, so a browser reading through sliced blobs and a command
//! line reading a file cannot disagree about what a file contains. The number of bytes a read cost is
//! observable through [`CountingSource`], which is how the inspection budget is tested rather than assumed.
//!
//! Nothing here panics on any input. Sizes are checked, depth and count are capped, and an inconsistency is
//! an [`Error`] that names the box and the offset.
//!
//! Writing follows the same rule. [`remux()`] rewrites a container around samples it never touches: the
//! kept tracks' boxes are copied as the bytes they were read as, only the chunk offset tables are written
//! afresh, the user-data box and the unknown top-level boxes are carried across, and the movie box comes
//! first so the output plays as it arrives.

#![forbid(unsafe_code)]

mod error;
mod fourcc;
mod parse;
mod remux;
mod samples;
mod source;
mod tree;
mod views;

pub use error::Error;
pub use fourcc::FourCc;
pub use parse::{
    Container, FILE_TYPE_BOX_CAP, MAX_CHILDREN, MAX_DEPTH, MAX_TOP_LEVEL, MOVIE_BOX_CAP, Parser, Request,
    Step, UNKNOWN_BOX_HOLD_CAP, parse, parse_async,
};
pub use remux::{Input, Placement, PreservedBox, Progress, RemuxPlan, RemuxReport, RemuxedTrack, remux};
pub use samples::{MAX_SAMPLE_SIZE, Sample, SampleTable, Sizes};
pub use source::{AsyncSource, CountingSource, FileSource, ReadRange, SliceSource, Source, SourceError};
pub use tree::BoxRange;
pub use views::{
    AudioFields, AvcConfiguration, Ctts, Elst, Ftyp, Hdlr, Mdhd, Mvhd, SampleEntry, Stsd, Stss, Tkhd, Track,
    TrackKind, VisualFields,
};
