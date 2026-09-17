//! **G52.** Every count and every table in the documentation matches the repository.
//!
//! This project's whole argument is that a claim without a check behind it is worth nothing. A number written
//! in a README is a claim, and it is the claim most likely to quietly stop being true: a guarantee is added,
//! a probe is recorded, and a word like "forty" stays where it was.
//!
//! Six such claims are held here. The ledger accounts for every guarantee identifier exactly once. The
//! enforced tables list exactly the guarantees the suite holds, and each of those is a real test. The
//! README's count of committed guarantees and of probes match the ledger and the register. The number of
//! decision records the changelog states matches the records on disk. And the gate sequence the
//! documentation describes is exactly the sequence the script runs, in both directions.
//!
//! The plan scheduled this for the last milestone, to reconcile the tables once everything was written. It
//! arrived at the first one instead, because a count in a document is wrong the moment nobody checks it.

use adiungere_cli::evidence::Register;
use adiungere_guarantees::{TextFile, read_at, tracked_paths, tracked_text_files};
use std::collections::{BTreeMap, BTreeSet};

/// The highest guarantee identifier the ledger accounts for.
const HIGHEST: u32 = 54;

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

/// How many times each identifier is listed, read from rows beginning with a guarantee cell.
fn guarantee_rows_in(table: &str) -> BTreeMap<u32, usize> {
    let mut counts = BTreeMap::new();

    for line in table.lines() {
        let Some(cell) = line.strip_prefix("| G") else {
            continue;
        };
        let digits: String = cell.chars().take_while(char::is_ascii_digit).collect();

        if let Ok(id) = digits.parse::<u32>() {
            *counts.entry(id).or_insert(0) += 1;
        }
    }

    counts
}

/// The part of a document between one heading and the next of the same level.
fn section_of(document: &str, heading: &str) -> String {
    let level = heading.chars().take_while(|c| *c == '#').count();
    let marker = format!("{} ", "#".repeat(level));
    let mut collecting = false;
    let mut collected = Vec::new();

    for line in document.lines() {
        if line.trim() == heading {
            collecting = true;
            continue;
        }

        if collecting && line.starts_with(&marker) {
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

/// The names of the steps the gate script runs, in order.
fn gate_steps(script: &str) -> Vec<String> {
    script
        .lines()
        .filter_map(|line| {
            let rest = line.trim().strip_prefix("run '")?;
            let (name, _) = rest.split_once('\'')?;

            Some(name.to_owned())
        })
        .collect()
}

/// The steps the testing document says run today: rows whose command column is a command, not a milestone.
fn documented_steps(testing: &str) -> Vec<String> {
    section_of(testing, "## The gate sequence")
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("| #") && !line.starts_with("|---"))
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let name = cells.get(2)?;
            let command = cells.get(3)?;

            (command.contains('`')).then(|| name.to_lowercase())
        })
        .collect()
}

#[test]
fn the_ledger_accounts_for_every_identifier_exactly_once() {
    // Arrange
    let ledger = read_at("docs/guarantees.md").unwrap();

    // Act
    let rows = guarantee_rows_in(&ledger);
    let expected: BTreeSet<u32> = (1..=HIGHEST).collect();
    let listed: BTreeSet<u32> = rows.keys().copied().collect();
    let missing: Vec<u32> = expected.difference(&listed).copied().collect();
    let unexpected: Vec<u32> = listed.difference(&expected).copied().collect();
    let repeated: Vec<u32> = rows
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(id, _)| *id)
        .collect();

    // Assert
    assert!(
        missing.is_empty(),
        "The ledger accounts for no such identifiers: {missing:?}"
    );
    assert!(
        unexpected.is_empty(),
        "The ledger lists identifiers beyond the range: {unexpected:?}"
    );
    assert!(
        repeated.is_empty(),
        "The ledger lists these identifiers more than once: {repeated:?}"
    );
}

#[test]
fn the_enforced_tables_list_exactly_the_guarantees_the_suite_holds() {
    // Arrange
    let ledger = read_at("docs/guarantees.md").unwrap();
    let readme = read_at("README.md").unwrap();
    let paths = tracked_paths().unwrap();

    // Act
    let held = guarantees_in_the_suite(&paths);
    let in_ledger: BTreeSet<u32> = guarantee_rows_in(&section_of(&ledger, "## Enforced today"))
        .into_keys()
        .collect();
    let in_readme: BTreeSet<u32> = guarantee_rows_in(&section_of(&readme, "### Enforced today"))
        .into_keys()
        .collect();

    // Assert
    assert!(
        !held.is_empty(),
        "No guarantee test file was found; this check is inert."
    );
    assert_eq!(
        in_ledger, held,
        "The ledger's enforced table and the suite disagree."
    );
    assert_eq!(
        in_readme, held,
        "The README's enforced table and the suite disagree."
    );
}

#[test]
fn every_guarantee_file_in_the_suite_holds_a_test() {
    // A file named like a guarantee and holding no test would count as enforced while enforcing nothing.

    // Arrange
    let files: Vec<TextFile> = tracked_text_files()
        .unwrap()
        .into_iter()
        .filter(|file| file.path.starts_with("core/crates/adiungere-guarantees/tests/g"))
        .collect();

    // Act
    let empty: Vec<String> = files
        .iter()
        .filter(|file| !file.has_extension("rs") || !file.contents.contains("#[test]"))
        .map(|file| file.path.clone())
        .collect();

    // Assert
    assert!(
        !files.is_empty(),
        "No guarantee file was read; this check is inert."
    );
    assert!(
        empty.is_empty(),
        "These guarantee files hold no test:\n  {}",
        empty.join("\n  ")
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
fn the_changelog_counts_the_decision_records_correctly() {
    // Arrange
    let changelog = read_at("CHANGELOG.md").unwrap();
    let records = tracked_paths()
        .unwrap()
        .iter()
        .filter(|path| {
            path.starts_with("docs/adr/")
                && adiungere_guarantees::is_markdown_path(path)
                && !path.ends_with("README.md")
                && !path.ends_with("0000-template.md")
        })
        .count();

    // Act
    let expected = format!("{} decision records", number_word(records));

    // Assert
    assert!(
        changelog.to_lowercase().contains(&expected),
        "The changelog should say \"{expected}\", because that many records exist."
    );
}

#[test]
fn the_documented_gate_sequence_is_the_sequence_the_script_runs() {
    // A gate the documentation describes and the script does not run is a step nobody performs while everyone
    // believes it happens, and a step the script runs that the documentation omits is one nobody can
    // reproduce from the text. Both directions, in order.

    // Arrange
    let script = read_at("scripts/gate.sh").unwrap();
    let testing = read_at("docs/testing.md").unwrap();

    // Act
    let run: Vec<String> = gate_steps(&script)
        .iter()
        .map(|step| step.to_lowercase())
        .collect();
    let documented = documented_steps(&testing);

    // Assert
    assert!(
        run.len() >= 8,
        "Only {} steps were read from the script; this check is inert.",
        run.len()
    );
    assert_eq!(
        run, documented,
        "The steps the script runs and the steps the testing document lists as running today must be the \
         same, in the same order."
    );
}

#[test]
fn the_number_words_are_the_ones_this_project_writes() {
    // The comparison above is only as good as the spelling, so the spelling is exercised.

    // Act and assert
    assert_eq!(number_word(7), "seven");
    assert_eq!(number_word(11), "eleven");
    assert_eq!(number_word(14), "fourteen");
    assert_eq!(number_word(39), "thirty-nine");
    assert_eq!(number_word(40), "forty");
    assert_eq!(number_word(41), "forty-one");
}

#[test]
fn the_section_reader_stops_at_the_next_heading_of_the_same_level() {
    // Without this, the enforced table would be compared against the whole document and would always agree.

    // Arrange
    let document = "## Enforced today\n\n| G01 | a |\n\n### A sub-heading\n\n| G02 | b |\n\n## Committed\n\n| G12 | c |\n";

    // Act
    let enforced: BTreeSet<u32> = guarantee_rows_in(&section_of(document, "## Enforced today"))
        .into_keys()
        .collect();

    // Assert
    assert_eq!(enforced, [1, 2].into_iter().collect::<BTreeSet<u32>>());
}

#[test]
fn a_repeated_row_is_counted_twice() {
    // Act
    let rows = guarantee_rows_in("| G12 | a |\n| G12 | b |\n| G13 | c |\n");

    // Assert
    assert_eq!(rows.get(&12), Some(&2));
    assert_eq!(rows.get(&13), Some(&1));
}
