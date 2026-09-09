//! The evidence register: probes, verdicts, and the rules that keep the file honest.
//!
//! Every load-bearing assumption in this project is a probe: a question, the method that answers it, and the
//! decision it settles. `docs/evidence.md` records them, and this module reads that file as data.
//!
//! The parser is deliberately strict. A lenient parser would skip an entry whose shape had drifted and
//! report fewer probes than the file contains, which is exactly the silent under-reporting the register
//! exists to prevent. Three rules are worth naming because they are what make a verdict mean something:
//!
//! - Entries are in ascending order, so the file cannot grow two entries for one identifier.
//! - The bullets are in a fixed order, so an entry is either complete or refused.
//! - A verdict of `measured` or `reasoned` **must** carry a result, and `unavailable` or `not started`
//!   **must not**. A measurement with no recorded finding is not a measurement.

use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;

use serde::{Serialize, Serializer};

use crate::roadmap::{MilestoneId, Roadmap};

/// The identifier of a probe, written `P01` in the register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProbeId(u16);

impl ProbeId {
    /// The number behind the identifier.
    #[must_use]
    pub const fn number(self) -> u16 {
        self.0
    }
}

impl fmt::Display for ProbeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "P{:02}", self.0)
    }
}

impl FromStr for ProbeId {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let digits = value.strip_prefix('P').ok_or(())?;

        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(());
        }

        digits.parse::<u16>().map(ProbeId).map_err(|_| ())
    }
}

impl Serialize for ProbeId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// What is known about a probe, in the only four words the register admits.
///
/// `Unavailable` and `NotStarted` are never read as a pass. A decision that depends on an unanswered probe
/// carries its fallback in the open, or it waits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Verdict {
    /// The method was run and produced a result, which the entry records.
    Measured,
    /// No measurement was possible, and the conclusion rests on a primary source the entry quotes.
    Reasoned,
    /// The method exists and could not be run. The entry says what is missing.
    Unavailable,
    /// Nobody has run it. It is not evidence of anything.
    NotStarted,
}

impl Verdict {
    /// Every verdict, in the order the register documents them.
    pub const ALL: [Self; 4] = [
        Self::Measured,
        Self::Reasoned,
        Self::Unavailable,
        Self::NotStarted,
    ];

    /// The word this verdict is written with.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Measured => "measured",
            Self::Reasoned => "reasoned",
            Self::Unavailable => "unavailable",
            Self::NotStarted => "not started",
        }
    }

    /// Whether this verdict is a finding, and therefore has to record a result.
    #[must_use]
    pub const fn is_a_finding(self) -> bool {
        matches!(self, Self::Measured | Self::Reasoned)
    }
}

impl fmt::Display for Verdict {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl FromStr for Verdict {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|verdict| verdict.as_str() == value)
            .ok_or(())
    }
}

impl Serialize for Verdict {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// One entry of the register.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Probe {
    /// The identifier, unique and ascending through the file.
    pub id: ProbeId,
    /// The heading, which says in one line what the probe is about.
    pub title: String,
    /// What is known today.
    pub verdict: Verdict,
    /// The milestone that has to carry the answer.
    pub milestone: MilestoneId,
    /// What is unknown, phrased so that an answer can be wrong.
    pub question: String,
    /// What would be run, precisely enough that someone else could run it.
    pub method: String,
    /// The decision, guarantee or milestone item that changes with the answer.
    pub decides: String,
    /// The finding, present exactly when the verdict is a finding.
    pub result: Option<String>,
    /// The line the entry starts on, so a failure can point at the file.
    pub line: usize,
}

/// The register as a whole.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Register {
    probes: Vec<Probe>,
}

impl Register {
    /// Reads a register from the text of `docs/evidence.md`.
    ///
    /// # Errors
    ///
    /// Returns the first structural problem found, with the line it is on. An entry is never skipped: a
    /// register that cannot be read completely is refused, because reading it partially would under-report
    /// the very thing it exists to record.
    pub fn parse(source: &str) -> Result<Self, ParseError> {
        let mut probes: Vec<Probe> = Vec::new();
        let mut pending: Option<Pending> = None;
        let mut inside_fence = false;

        for (index, raw) in source.lines().enumerate() {
            let line = index + 1;
            let text = raw.trim_end();

            if text.trim_start().starts_with("```") {
                inside_fence = !inside_fence;
                continue;
            }

            if inside_fence {
                continue;
            }

            if let Some(rest) = text.strip_prefix("## ") {
                if let Some(entry) = pending.take() {
                    probes.push(entry.finish()?);
                }

                if let Some((id, title)) = split_heading(rest) {
                    if let Some(previous) = probes.last()
                        && id <= previous.id
                    {
                        return Err(ParseError::OutOfOrder {
                            line,
                            id,
                            previous: previous.id,
                        });
                    }

                    pending = Some(Pending::new(id, title, line));
                }

                continue;
            }

            if text.starts_with('#') {
                if let Some(entry) = pending.take() {
                    probes.push(entry.finish()?);
                }

                continue;
            }

            let Some(entry) = pending.as_mut() else {
                continue;
            };

            entry.absorb(text, line)?;
        }

        if let Some(entry) = pending.take() {
            probes.push(entry.finish()?);
        }

        if probes.is_empty() {
            return Err(ParseError::Empty);
        }

        Ok(Self { probes })
    }

    /// Every probe, in file order.
    #[must_use]
    pub fn probes(&self) -> &[Probe] {
        &self.probes
    }

    /// The probe with this identifier, if the register holds one.
    #[must_use]
    pub fn get(&self, id: ProbeId) -> Option<&Probe> {
        self.probes.iter().find(|probe| probe.id == id)
    }

    /// How many probes carry each verdict, in the order the register documents the verdicts.
    #[must_use]
    pub fn tally(&self) -> [(Verdict, usize); 4] {
        Verdict::ALL.map(|verdict| {
            (
                verdict,
                self.probes
                    .iter()
                    .filter(|probe| probe.verdict == verdict)
                    .count(),
            )
        })
    }

    /// Compares the register with the roadmap and reports every way the two disagree.
    ///
    /// An empty result means each probe names a milestone the roadmap has, each milestone names the probes
    /// assigned to it, and the roadmap mentions no probe the register has never heard of.
    #[must_use]
    pub fn reconcile(&self, roadmap: &Roadmap) -> Vec<Discrepancy> {
        let mut found = Vec::new();

        for probe in &self.probes {
            match roadmap.section(probe.milestone) {
                None => found.push(Discrepancy::UnknownMilestone {
                    id: probe.id,
                    milestone: probe.milestone,
                }),
                Some(section) => {
                    if !section.probe_mentions().contains(&probe.id) {
                        found.push(Discrepancy::NotMentionedInItsMilestone {
                            id: probe.id,
                            milestone: probe.milestone,
                        });
                    }
                },
            }
        }

        for section in roadmap.sections() {
            for &id in section.probe_mentions() {
                if self.get(id).is_none() {
                    found.push(Discrepancy::MentionedButNotRegistered {
                        id,
                        milestone: section.id,
                    });
                }
            }
        }

        found
    }
}

/// A way in which the register and the roadmap disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Discrepancy {
    /// A probe names a milestone the roadmap does not have.
    UnknownMilestone {
        /// The probe.
        id: ProbeId,
        /// The milestone it names.
        milestone: MilestoneId,
    },
    /// A probe is assigned to a milestone whose section never mentions it.
    NotMentionedInItsMilestone {
        /// The probe.
        id: ProbeId,
        /// The milestone that should mention it.
        milestone: MilestoneId,
    },
    /// The roadmap mentions a probe the register does not hold.
    MentionedButNotRegistered {
        /// The probe named in the roadmap.
        id: ProbeId,
        /// The milestone whose section names it.
        milestone: MilestoneId,
    },
}

impl fmt::Display for Discrepancy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownMilestone { id, milestone } => write!(
                formatter,
                "{id} is assigned to {milestone}, which the roadmap does not have"
            ),
            Self::NotMentionedInItsMilestone { id, milestone } => write!(
                formatter,
                "{id} is assigned to {milestone}, and the {milestone} section never mentions it"
            ),
            Self::MentionedButNotRegistered { id, milestone } => {
                write!(
                    formatter,
                    "{milestone} mentions {id}, which the register does not hold"
                )
            },
        }
    }
}

/// Everything that can be wrong with the register's structure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    /// The file holds no entry at all, so reading it proved nothing.
    Empty,
    /// An entry appears before one with a lower or equal identifier.
    OutOfOrder {
        /// The line of the offending heading.
        line: usize,
        /// The identifier that arrived out of order.
        id: ProbeId,
        /// The identifier before it.
        previous: ProbeId,
    },
    /// A bullet names something that is not a field of an entry.
    UnknownField {
        /// The line it is on.
        line: usize,
        /// The label that was read.
        label: String,
    },
    /// A field appears twice in one entry.
    RepeatedField {
        /// The line of the second one.
        line: usize,
        /// The field.
        field: &'static str,
        /// The entry it belongs to.
        id: ProbeId,
    },
    /// The fields of an entry are not in the documented order.
    FieldOutOfOrder {
        /// The line of the field that arrived early or late.
        line: usize,
        /// The field that was read.
        field: &'static str,
        /// The field that was expected there.
        expected: &'static str,
        /// The entry it belongs to.
        id: ProbeId,
    },
    /// An entry lacks a field it must have.
    MissingField {
        /// The entry.
        id: ProbeId,
        /// The field.
        field: &'static str,
    },
    /// A field is present and says nothing.
    EmptyField {
        /// The line it is on.
        line: usize,
        /// The field.
        field: &'static str,
    },
    /// The verdict is not one of the four words.
    UnknownVerdict {
        /// The line it is on.
        line: usize,
        /// What was written there.
        value: String,
    },
    /// The milestone is not written as a milestone identifier.
    MalformedMilestone {
        /// The line it is on.
        line: usize,
        /// What was written there.
        value: String,
    },
    /// A verdict that is not a finding carries a result.
    ResultWithoutFinding {
        /// The entry.
        id: ProbeId,
        /// Its verdict.
        verdict: Verdict,
    },
    /// A verdict that is a finding carries no result.
    FindingWithoutResult {
        /// The entry.
        id: ProbeId,
        /// Its verdict.
        verdict: Verdict,
    },
    /// A line inside an entry is neither a bullet, a continuation, nor part of a result.
    StrayLine {
        /// The line it is on.
        line: usize,
        /// The entry it landed in.
        id: ProbeId,
        /// What was written there.
        text: String,
    },
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter
                .write_str("the register holds no entry, so reading it proved nothing about any probe"),
            Self::OutOfOrder { line, id, previous } => write!(
                formatter,
                "line {line}: {id} comes after {previous}; entries are in ascending order"
            ),
            Self::UnknownField { line, label } => {
                write!(formatter, "line {line}: an entry has no field called \"{label}\"")
            },
            Self::RepeatedField { line, field, id } => {
                write!(formatter, "line {line}: {id} states \"{field}\" twice")
            },
            Self::FieldOutOfOrder {
                line,
                field,
                expected,
                id,
            } => write!(
                formatter,
                "line {line}: {id} states \"{field}\" where \"{expected}\" was expected; the fields are in a \
                 fixed order"
            ),
            Self::MissingField { id, field } => {
                write!(formatter, "{id} has no \"{field}\" field")
            },
            Self::EmptyField { line, field } => {
                write!(formatter, "line {line}: \"{field}\" is present and says nothing")
            },
            Self::UnknownVerdict { line, value } => write!(
                formatter,
                "line {line}: \"{value}\" is not a verdict; use measured, reasoned, unavailable or not started"
            ),
            Self::MalformedMilestone { line, value } => {
                write!(
                    formatter,
                    "line {line}: \"{value}\" is not a milestone identifier such as M1"
                )
            },
            Self::ResultWithoutFinding { id, verdict } => write!(
                formatter,
                "{id} is {verdict} and records a result; only a measurement or a reasoned conclusion may"
            ),
            Self::FindingWithoutResult { id, verdict } => write!(
                formatter,
                "{id} is {verdict} and records no result; a finding with nothing written down is not a finding"
            ),
            Self::StrayLine { line, id, text } => write!(
                formatter,
                "line {line}: {id} carries a line that is neither a field nor a continuation: \"{text}\""
            ),
        }
    }
}

impl std::error::Error for ParseError {}

/// The fields of an entry, in the order they must appear.
const FIELD_ORDER: [&str; 6] = ["Verdict", "Milestone", "Question", "Method", "Decides", "Result"];

#[derive(Debug)]
struct Pending {
    id: ProbeId,
    title: String,
    line: usize,
    fields: Vec<(usize, String, usize)>,
    result: Vec<String>,
    result_open: bool,
}

impl Pending {
    fn new(id: ProbeId, title: String, line: usize) -> Self {
        Self {
            id,
            title,
            line,
            fields: Vec::new(),
            result: Vec::new(),
            result_open: false,
        }
    }

    fn absorb(&mut self, text: &str, line: usize) -> Result<(), ParseError> {
        if let Some((label, value)) = split_bullet(text) {
            return self.set(label, value, line);
        }

        if self.result_open {
            self.result
                .push(text.strip_prefix("  ").unwrap_or(text).to_owned());
            return Ok(());
        }

        if text.trim().is_empty() {
            return Ok(());
        }

        if let Some(continuation) = text.strip_prefix("  ")
            && let Some(last) = self.fields.last_mut()
        {
            last.1.push(' ');
            last.1.push_str(continuation.trim());
            return Ok(());
        }

        Err(ParseError::StrayLine {
            line,
            id: self.id,
            text: text.trim().to_owned(),
        })
    }

    fn set(&mut self, label: &str, value: &str, line: usize) -> Result<(), ParseError> {
        let Some((index, name)) = FIELD_ORDER
            .iter()
            .copied()
            .enumerate()
            .find(|(_, known)| *known == label)
        else {
            return Err(ParseError::UnknownField {
                line,
                label: label.to_owned(),
            });
        };

        if self.fields.iter().any(|(seen, _, _)| *seen == index) {
            return Err(ParseError::RepeatedField {
                line,
                field: name,
                id: self.id,
            });
        }

        if index != self.fields.len() {
            let expected = FIELD_ORDER
                .get(self.fields.len())
                .copied()
                .unwrap_or("nothing further");
            return Err(ParseError::FieldOutOfOrder {
                line,
                field: name,
                expected,
                id: self.id,
            });
        }

        if value.trim().is_empty() {
            return Err(ParseError::EmptyField { line, field: name });
        }

        if name == "Result" {
            self.result_open = true;
            self.result.push(value.trim().to_owned());
        }

        self.fields.push((index, value.trim().to_owned(), line));

        Ok(())
    }

    fn field(&self, index: usize) -> Option<(&str, usize)> {
        self.fields
            .iter()
            .find(|(seen, _, _)| *seen == index)
            .map(|(_, value, line)| (value.as_str(), *line))
    }

    fn required(&self, index: usize) -> Result<(&str, usize), ParseError> {
        let name = FIELD_ORDER.get(index).copied().unwrap_or("a field");
        self.field(index).ok_or(ParseError::MissingField {
            id: self.id,
            field: name,
        })
    }

    fn finish(self) -> Result<Probe, ParseError> {
        let (verdict_text, verdict_line) = self.required(0)?;
        let verdict = verdict_text
            .parse::<Verdict>()
            .map_err(|()| ParseError::UnknownVerdict {
                line: verdict_line,
                value: verdict_text.to_owned(),
            })?;

        let (milestone_text, milestone_line) = self.required(1)?;
        let milestone =
            milestone_text
                .parse::<MilestoneId>()
                .map_err(|()| ParseError::MalformedMilestone {
                    line: milestone_line,
                    value: milestone_text.to_owned(),
                })?;

        let question = self.required(2)?.0.to_owned();
        let method = self.required(3)?.0.to_owned();
        let decides = self.required(4)?.0.to_owned();

        let result = if self.result_open {
            let joined = self.result.join("\n").trim().to_owned();
            Some(joined)
        } else {
            None
        };

        match (verdict.is_a_finding(), result.is_some()) {
            (false, true) => return Err(ParseError::ResultWithoutFinding { id: self.id, verdict }),
            (true, false) => return Err(ParseError::FindingWithoutResult { id: self.id, verdict }),
            _ => {},
        }

        Ok(Probe {
            id: self.id,
            title: self.title,
            verdict,
            milestone,
            question,
            method,
            decides,
            result,
            line: self.line,
        })
    }
}

/// Splits `P01 A title` into its identifier and its title, canonical spelling required.
fn split_heading(rest: &str) -> Option<(ProbeId, String)> {
    let (candidate, title) = rest.split_once(' ')?;
    let id = candidate.parse::<ProbeId>().ok()?;

    if id.to_string() != candidate {
        return None;
    }

    let title = title.trim();

    if title.is_empty() {
        None
    } else {
        Some((id, title.to_owned()))
    }
}

/// Splits `- **Verdict:** measured` into its label and its value.
fn split_bullet(text: &str) -> Option<(&str, &str)> {
    let rest = text.strip_prefix("- **")?;
    let (label, value) = rest.split_once("**")?;
    Some((label.strip_suffix(':')?, value.trim_start()))
}

/// Every probe identifier mentioned in a piece of prose, as whole tokens.
///
/// A token counts only when it is not glued to a letter or a digit on either side, so an identifier inside a
/// longer word is not a mention.
#[must_use]
pub fn probe_mentions(text: &str) -> BTreeSet<ProbeId> {
    let mut found = BTreeSet::new();
    let bytes = text.as_bytes();

    for (start, _) in text.char_indices().filter(|(_, character)| *character == 'P') {
        let preceded_by_word = start
            .checked_sub(1)
            .and_then(|before| bytes.get(before))
            .is_some_and(u8::is_ascii_alphanumeric);

        if preceded_by_word {
            continue;
        }

        let digits = bytes
            .iter()
            .skip(start + 1)
            .take_while(|byte| byte.is_ascii_digit())
            .count();

        if digits < 2 {
            continue;
        }

        let followed_by_word = bytes
            .get(start + 1 + digits)
            .is_some_and(u8::is_ascii_alphanumeric);

        if followed_by_word {
            continue;
        }

        if let Some(token) = text.get(start..start + 1 + digits)
            && let Ok(id) = token.parse::<ProbeId>()
            && id.to_string() == token
        {
            found.insert(id);
        }
    }

    found
}
