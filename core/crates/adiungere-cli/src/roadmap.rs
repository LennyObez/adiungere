//! The roadmap, read as data.
//!
//! `docs/roadmap.md` is the contract between the milestones and everything else in the repository. This
//! module reads its milestone headings and the text under each of them, which is what makes it possible to
//! ask whether a milestone actually names the probes assigned to it.
//!
//! Only the structure is read. What a milestone promises is prose, and prose is for people.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Serialize, Serializer};

use crate::evidence::{ProbeId, probe_mentions};

/// The identifier of a milestone, written `M0` in the roadmap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MilestoneId(u8);

impl MilestoneId {
    /// The number behind the identifier.
    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }
}

impl fmt::Display for MilestoneId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "M{}", self.0)
    }
}

impl FromStr for MilestoneId {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digits = value.strip_prefix('M').ok_or(())?;

        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(());
        }

        let number = digits.parse::<u8>().map_err(|_| ())?;

        if number.to_string() == digits {
            Ok(Self(number))
        } else {
            Err(())
        }
    }
}

impl Serialize for MilestoneId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// One milestone of the roadmap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    /// The identifier.
    pub id: MilestoneId,
    /// The heading, without the identifier.
    pub title: String,
    /// Everything written under the heading.
    pub body: String,
    mentions: BTreeSet<ProbeId>,
}

impl Section {
    /// The probes this milestone names in its own text.
    #[must_use]
    pub const fn probe_mentions(&self) -> &BTreeSet<ProbeId> {
        &self.mentions
    }
}

/// The roadmap as a whole.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Roadmap {
    sections: Vec<Section>,
}

impl Roadmap {
    /// Reads a roadmap from the text of `docs/roadmap.md`.
    ///
    /// # Errors
    ///
    /// Returns an error when the file names no milestone, or names them out of order. Both would make a
    /// reconciliation against the register meaningless while looking like it had run.
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let mut sections: Vec<(MilestoneId, String, usize, Vec<&str>)> = Vec::new();
        let mut inside_fence = false;

        for (index, raw) in source.lines().enumerate() {
            let line = index + 1;
            let text = raw.trim_end();

            if text.trim_start().starts_with("```") {
                inside_fence = !inside_fence;
            }

            if !inside_fence && let Some(rest) = text.strip_prefix("## ") {
                if let Some((id, title)) = split_heading(rest) {
                    if let Some((previous, _, _, _)) = sections.last()
                        && id <= *previous
                    {
                        return Err(ParseError::OutOfOrder {
                            line,
                            id,
                            previous: *previous,
                        });
                    }

                    sections.push((id, title, line, Vec::new()));
                    continue;
                }

                if let Some((_, _, _, body)) = sections.last_mut() {
                    body.push(text);
                }

                continue;
            }

            if let Some((_, _, _, body)) = sections.last_mut() {
                body.push(text);
            }
        }

        if sections.is_empty() {
            return Err(ParseError::Empty);
        }

        let sections = sections
            .into_iter()
            .map(|(id, title, _, body)| {
                let body = body.join("\n");
                let mentions = probe_mentions(&body);
                Section {
                    id,
                    title,
                    body,
                    mentions,
                }
            })
            .collect();

        Ok(Self { sections })
    }

    /// Every milestone, in file order.
    #[must_use]
    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// The milestone with this identifier, if the roadmap has one.
    #[must_use]
    pub fn section(&self, id: MilestoneId) -> Option<&Section> {
        self.sections.iter().find(|section| section.id == id)
    }
}

/// Everything that can be wrong with the roadmap's structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// The file names no milestone, so there is nothing to reconcile against.
    Empty,
    /// A milestone appears before one with a lower or equal identifier.
    OutOfOrder {
        /// The line of the offending heading.
        line: usize,
        /// The identifier that arrived out of order.
        id: MilestoneId,
        /// The identifier before it.
        previous: MilestoneId,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => {
                formatter.write_str("the roadmap names no milestone, so nothing can be reconciled against it")
            },
            Self::OutOfOrder { line, id, previous } => write!(
                formatter,
                "line {line}: {id} comes after {previous}; milestones are in ascending order"
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// Splits `M0: Foundation` into its identifier and its title.
fn split_heading(rest: &str) -> Option<(MilestoneId, String)> {
    let (candidate, title) = rest.split_once(':')?;
    let id = candidate.parse::<MilestoneId>().ok()?;
    let title = title.trim();

    if title.is_empty() {
        None
    } else {
        Some((id, title.to_owned()))
    }
}
