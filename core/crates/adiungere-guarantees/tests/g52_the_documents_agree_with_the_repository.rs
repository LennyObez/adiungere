//! **G52.** Every count and every table in the documentation matches the repository.
//!
//! This project's whole argument is that a claim without a check behind it is worth nothing. A number written
//! in a README is a claim, and it is the claim most likely to quietly stop being true: a guarantee is added,
//! a probe is recorded, and a word like "forty" stays where it was.
//!
//! Four such claims are held here. The ledger accounts for every guarantee identifier exactly once. The
//! enforced table lists exactly the guarantees the suite holds, no more and no fewer. The README's count of
//! committed guarantees matches the ledger. And the gate sequence the documentation describes is the sequence
//! the script runs.
//!
//! The plan scheduled this for the last milestone, to reconcile the tables once everything was written. It
//! arrived at the first one instead, because it caught a wrong number in the README on the day it was written.

use adiungere_cli::evidence::Register;
use adiungere_guarantees::{read_at, tracked_paths};
use std::collections::BTreeSet;

/// The highest guarantee identifier the ledger accounts for.
const HIGHEST: u32 = 52;

/// Identifiers of the guarantees the suite actually holds, read from the test file names.
fn guarantees_in_the_suite(paths: &[String]) -> BTreeSet<u32> {
    paths
        .iter()
        .filter_map(|path| {
            let name = path.strip_prefix("core/crates/adiungere-guarantees/tests/g")?;
            let digits: String = name.chars().take_while(char::is_ascii_digit).collect();

            digits.parse().ok()
        })
        .collect()
}

/// Identifiers a table lists, read from rows beginning with a guarantee cell.
fn guarantees_listed_in(table: &str) -> BTreeSet<u32> {
    table
        .lines()
        .filter_map(|line| {
            let cell = line.strip_prefix("| G")?;
            let digits: String = cell.chars().take_while(char::is_ascii_digit).collect();

            digits.parse().ok()
        })
        .collect()
}

/// The part of a document between one heading and the next of the same level.
fn section_of(document: &str, heading: &str) -> String {
    let mut collecting = false;
    let mut collected = Vec::new();

    for line in document.lines() {
        if line.trim() == heading {
            collecting = true;
            continue;
        }

        if collecting && line.starts_with("## ") {
            break;
        }

        if collecting {
            collected.push(line);
        }
    }

    collected.join("\n")
}

/// The number words this project writes out, up to the size the ledger can reach.
fn number_word(value: usize) -> String {
    const UNITS: [&str; 10] = [
        "zero", "one", "two", "three", "four", "five", "six", "seven", "eight", "nine",
    ];
    const TEENS: [&str; 10] = [
        "ten",
        "eleven",
        "twelve",
        "thirteen",
        "fourteen",
        "fifteen",
        "sixteen",
        "seventeen",
        "eighteen",
        "nineteen",
    ];
    const TENS: [&str; 6] = ["twenty", "thirty", "forty", "fifty", "sixty", "seventy"];

    if let Some(word) = UNITS.get(value) {
        return (*word).to_owned();
    }

    if let Some(word) = value.checked_sub(10).and_then(|index| TEENS.get(index)) {
        return (*word).to_owned();
    }

    // Counted rather than divided. Integer division is denied workspace-wide, because in this product it
    // appears where a timescale is converted and that is exactly where a precision defect lives. A loop over
    // two digits is not worth an exception to the rule.
    let mut tens = 0usize;
    let mut unit = value;

    while unit >= 10 {
        unit -= 10;
        tens += 1;
    }

    let Some(tens_word) = tens.checked_sub(2).and_then(|index| TENS.get(index)) else {
        return value.to_string();
    };

    if unit == 0 {
        (*tens_word).to_owned()
    } else {
        format!("{tens_word}-{}", UNITS.get(unit).copied().unwrap_or_default())
    }
}

#[test]
fn the_ledger_accounts_for_every_identifier_exactly_once() {
    // Arrange
    let ledger = read_at("docs/guarantees.md").unwrap();

    // Act
    let listed = guarantees_listed_in(&ledger);
    let expected: BTreeSet<u32> = (1..=HIGHEST).collect();
    let missing: Vec<u32> = expected.difference(&listed).copied().collect();
    let unexpected: Vec<u32> = listed.difference(&expected).copied().collect();

    // Assert
    assert!(
        missing.is_empty(),
        "The ledger accounts for no such identifiers: {missing:?}"
    );
    assert!(
        unexpected.is_empty(),
        "The ledger lists identifiers beyond the range: {unexpected:?}"
    );
}

#[test]
fn the_enforced_table_lists_exactly_the_guarantees_the_suite_holds() {
    // Arrange
    let ledger = read_at("docs/guarantees.md").unwrap();
    let readme = read_at("README.md").unwrap();
    let paths = tracked_paths().unwrap();

    // Act
    let held = guarantees_in_the_suite(&paths);
    let in_ledger = guarantees_listed_in(&section_of(&ledger, "## Enforced today"));
    let in_readme = guarantees_listed_in(&section_of(&readme, "### Enforced today"));

    // Assert
    assert!(
        !held.is_empty(),
        "No guarantee test file was found; this check is inert."
    );
    assert_eq!(
        in_ledger, held,
        "The ledger's enforced table and the suite disagree. Ledger: {in_ledger:?}, suite: {held:?}"
    );
    assert_eq!(
        in_readme, held,
        "The README's enforced table and the suite disagree. README: {in_readme:?}, suite: {held:?}"
    );
}

#[test]
fn the_readme_counts_the_committed_guarantees_correctly() {
    // Arrange
    let readme = read_at("README.md").unwrap();
    let paths = tracked_paths().unwrap();

    // Act
    let committed = HIGHEST as usize - guarantees_in_the_suite(&paths).len();
    let expected = format!("{} further guarantees", number_word(committed));

    // Assert
    assert!(
        readme.to_lowercase().contains(&expected),
        "The README should say \"{expected}\", because {committed} of the {HIGHEST} guarantees are not \
         enforced yet."
    );
}

#[test]
fn the_documented_gate_sequence_is_the_sequence_the_script_runs() {
    // A gate the documentation describes and the script does not run is a step nobody performs while everyone
    // believes it happens.

    // Arrange
    let script = read_at("scripts/gate.sh").unwrap();
    let testing = read_at("docs/testing.md").unwrap();

    // Act
    let steps: Vec<String> = script
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("run '")?;
            let (name, _) = rest.split_once('\'')?;

            Some(name.to_owned())
        })
        .collect();

    let undocumented: Vec<&String> = steps
        .iter()
        .filter(|step| !testing.to_lowercase().contains(&step.to_lowercase()))
        .collect();

    // Assert
    assert!(
        steps.len() >= 8,
        "Only {} steps were read from the script; this check is inert.",
        steps.len()
    );
    assert!(
        undocumented.is_empty(),
        "These gate steps run and the testing document does not describe them:\n  {}",
        undocumented
            .iter()
            .map(|step| step.as_str())
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn the_readme_counts_the_probes_correctly() {
    // The count comes from the product's own reader rather than from counting headings here. The register
    // documents its own format with a worked example inside a code block, and a second counter written for
    // this test read that example as a real entry. One parser, or two answers.

    // Arrange
    let readme = read_at("README.md").unwrap();
    let register = Register::parse(&read_at("docs/evidence.md").unwrap()).unwrap();

    // Act
    let entries = register.probes().len();

    // Assert
    assert!(entries > 0, "No probe was read; this check is inert.");
    assert!(
        readme.contains(&format!("{entries} of them")),
        "The README should say there are {entries} probes, which is what the register holds."
    );
}

#[test]
fn the_number_words_are_the_ones_this_project_writes() {
    // The comparison above is only as good as the spelling, so the spelling is exercised.

    // Act and assert
    assert_eq!(number_word(7), "seven");
    assert_eq!(number_word(11), "eleven");
    assert_eq!(number_word(12), "twelve");
    assert_eq!(number_word(39), "thirty-nine");
    assert_eq!(number_word(40), "forty");
    assert_eq!(number_word(41), "forty-one");
}

#[test]
fn the_section_reader_stops_at_the_next_heading() {
    // Without this, the enforced table would be compared against the whole document and would always agree.

    // Arrange
    let document = "## Enforced today\n\n| G01 | a |\n\n## Committed\n\n| G12 | b |\n";

    // Act
    let enforced = guarantees_listed_in(&section_of(document, "## Enforced today"));

    // Assert
    assert_eq!(enforced, [1].into_iter().collect::<BTreeSet<u32>>());
}
