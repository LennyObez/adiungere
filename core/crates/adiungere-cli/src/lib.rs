//! The adiungere command line, as a library.
//!
//! The command line is the executable specification of this product: everything a person can be told about a
//! file, a stranger can recompute here. This crate holds the parts of it that are worth testing on their own.
//!
//! At the current milestone that is the **evidence register**. Every load-bearing assumption in the project
//! is a probe with a verdict, recorded in `docs/evidence.md`, and the register is data as well as prose:
//! [`evidence::Register::parse`] reads it, [`roadmap::Roadmap::parse`] reads the milestones, and
//! [`evidence::Register::reconcile`] refuses a register that disagrees with the roadmap.
//!
//! Reading the register loudly matters more than reading it leniently. A parser that skipped an entry whose
//! shape had drifted would report fewer probes than exist, which is the failure this whole register is meant
//! to prevent.

pub mod evidence;
pub mod markdown;
pub mod media;
pub mod provenance;
pub mod repository;
pub mod roadmap;
