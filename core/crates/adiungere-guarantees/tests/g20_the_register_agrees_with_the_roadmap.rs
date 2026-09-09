//! **G20.** The evidence register and the roadmap say the same thing about every probe.
//!
//! The register holds the questions this project has not answered. The roadmap holds the milestones that have
//! to answer them. Either document on its own is a list; together they are a contract, and only if the two
//! halves agree.
//!
//! Two failures are being prevented. A probe assigned to a milestone whose section never mentions it is a
//! question nobody will be reminded of while doing the work that depends on it. A probe the roadmap names and
//! the register does not hold is a reference to a question nobody wrote down.
//!
//! This guarantee uses the product's own reader rather than a parser of its own. A second implementation would
//! agree with the first until the day it mattered.

use adiungere_cli::evidence::Register;
use adiungere_cli::roadmap::Roadmap;
use adiungere_guarantees::read_at;

#[test]
fn the_register_and_the_roadmap_agree() {
    // Arrange
    let register = Register::parse(&read_at("docs/evidence.md").unwrap()).unwrap();
    let roadmap = Roadmap::parse(&read_at("docs/roadmap.md").unwrap()).unwrap();

    // Act
    let discrepancies = register.reconcile(&roadmap);

    // Assert
    assert!(
        discrepancies.is_empty(),
        "A probe nobody can find from the roadmap is a question nobody will answer:\n  {}",
        discrepancies
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n  ")
    );
}

#[test]
fn every_answered_probe_records_what_was_found() {
    // The register's own parser enforces this, and it is restated here because it is the rule that makes a
    // verdict worth reading: anyone can write "measured".

    // Arrange
    let register = Register::parse(&read_at("docs/evidence.md").unwrap()).unwrap();

    // Act
    let silent: Vec<String> = register
        .probes()
        .iter()
        .filter(|probe| probe.verdict.is_a_finding() && probe.result.is_none())
        .map(|probe| probe.id.to_string())
        .collect();

    // Assert
    assert!(
        silent.is_empty(),
        "These probes claim a finding and record none: {silent:?}"
    );
}

#[test]
fn the_register_and_the_roadmap_were_really_read() {
    // Without this, a rename of either document would make the check above pass on an empty comparison.

    // Arrange
    let register = Register::parse(&read_at("docs/evidence.md").unwrap()).unwrap();
    let roadmap = Roadmap::parse(&read_at("docs/roadmap.md").unwrap()).unwrap();

    // Assert
    assert!(
        register.probes().len() >= 30,
        "Only {} probes were read; the check above is nearly inert.",
        register.probes().len()
    );
    assert_eq!(
        roadmap.sections().len(),
        11,
        "The roadmap should hold eleven milestones, M0 to M10."
    );
    assert!(
        roadmap
            .sections()
            .iter()
            .any(|section| !section.probe_mentions().is_empty()),
        "No milestone mentions a probe, so the comparison has nothing to compare."
    );
}
