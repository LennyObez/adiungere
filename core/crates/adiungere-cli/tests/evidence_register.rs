//! The evidence register is read strictly, or it is not read at all.
//!
//! These tests exist because the failure mode of a document parser is silence: it reads what it recognises,
//! ignores what it does not, and reports a smaller register than the file holds. Every test below is about
//! the parser refusing rather than about the parser succeeding.

use adiungere_cli::evidence::{Discrepancy, ParseError, Register, Verdict, probe_mentions};
use adiungere_cli::roadmap::Roadmap;

const PREAMBLE: &str = "# Evidence register\n\nProse that is not an entry.\n\n";

fn entry(id: &str, milestone: &str, verdict: &str) -> String {
    format!(
        "## {id} A title in sentence case\n\n\
         - **Verdict:** {verdict}\n\
         - **Milestone:** {milestone}\n\
         - **Question:** what is unknown.\n\
         - **Method:** what would be run.\n\
         - **Decides:** what changes with the answer.\n\n"
    )
}

fn register(body: &str) -> String {
    format!("{PREAMBLE}{body}")
}

#[test]
fn a_complete_entry_is_read_in_full() {
    // Arrange
    let source = register(&entry("P01", "M2", "not started"));

    // Act
    let parsed = Register::parse(&source).unwrap();

    // Assert
    let probe = parsed.probes().first().unwrap();
    assert_eq!(probe.id.to_string(), "P01");
    assert_eq!(probe.title, "A title in sentence case");
    assert_eq!(probe.verdict, Verdict::NotStarted);
    assert_eq!(probe.milestone.to_string(), "M2");
    assert_eq!(probe.question, "what is unknown.");
    assert_eq!(probe.result, None);
}

#[test]
fn a_wrapped_field_is_joined_back_into_one_sentence() {
    // The register wraps at 110 columns, so nearly every field spans several lines. A parser that took only
    // the first line would silently truncate the question it is meant to preserve.

    // Arrange
    let source = register(
        "## P01 A title\n\n\
         - **Verdict:** not started\n\
         - **Milestone:** M1\n\
         - **Question:** a question that runs\n  across two lines.\n\
         - **Method:** a method.\n\
         - **Decides:** a decision.\n",
    );

    // Act
    let parsed = Register::parse(&source).unwrap();

    // Assert
    assert_eq!(
        parsed.probes().first().unwrap().question,
        "a question that runs across two lines."
    );
}

#[test]
fn an_example_inside_a_code_fence_is_not_an_entry() {
    // The register documents its own format with a fenced example. Reading that example as a real entry
    // would put a probe in the register that nobody wrote.

    // Arrange
    let source = format!(
        "{PREAMBLE}```markdown\n## P99 An example\n\n- **Verdict:** measured\n```\n\n{}",
        entry("P01", "M1", "not started")
    );

    // Act
    let parsed = Register::parse(&source).unwrap();

    // Assert
    assert_eq!(parsed.probes().len(), 1);
    assert!(parsed.get("P99".parse().unwrap()).is_none());
}

#[test]
fn entries_out_of_order_are_refused() {
    // Ascending order is what stops the file from growing a second entry for one identifier in a place
    // nobody looks.

    // Arrange
    let source = register(&(entry("P02", "M1", "not started") + &entry("P01", "M1", "not started")));

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::OutOfOrder { .. })),
        "got {outcome:?}"
    );
}

#[test]
fn a_repeated_identifier_is_refused() {
    // Arrange
    let source = register(&(entry("P01", "M1", "not started") + &entry("P01", "M1", "not started")));

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::OutOfOrder { .. })),
        "got {outcome:?}"
    );
}

#[test]
fn a_missing_field_is_refused() {
    // Arrange
    let source = register(
        "## P01 A title\n\n\
         - **Verdict:** not started\n\
         - **Milestone:** M1\n\
         - **Question:** a question.\n\
         - **Method:** a method.\n",
    );

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::MissingField { field: "Decides", .. })),
        "got {outcome:?}"
    );
}

#[test]
fn fields_in_the_wrong_order_are_refused() {
    // A fixed order is what makes an entry readable at a glance and makes a missing field obvious.

    // Arrange
    let source = register(
        "## P01 A title\n\n\
         - **Milestone:** M1\n\
         - **Verdict:** not started\n\
         - **Question:** a question.\n\
         - **Method:** a method.\n\
         - **Decides:** a decision.\n",
    );

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::FieldOutOfOrder { .. })),
        "got {outcome:?}"
    );
}

#[test]
fn a_field_the_register_does_not_have_is_refused() {
    // Arrange
    let source = register(
        "## P01 A title\n\n\
         - **Verdict:** not started\n\
         - **Confidence:** high\n\
         - **Milestone:** M1\n\
         - **Question:** a question.\n\
         - **Method:** a method.\n\
         - **Decides:** a decision.\n",
    );

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::UnknownField { .. })),
        "got {outcome:?}"
    );
}

#[test]
fn a_word_that_is_not_a_verdict_is_refused() {
    // Four words, and no fifth. "probably fine" is how a register stops meaning anything.

    // Arrange
    let source = register(&entry("P01", "M1", "probably fine"));

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::UnknownVerdict { .. })),
        "got {outcome:?}"
    );
}

#[test]
fn a_measurement_with_no_recorded_finding_is_refused() {
    // This is the rule that makes a verdict worth reading. Anyone can write "measured"; the register
    // refuses it unless the finding is written down underneath.

    // Arrange
    let source = register(&entry("P01", "M1", "measured"));

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(
            outcome,
            Err(ParseError::FindingWithoutResult {
                verdict: Verdict::Measured,
                ..
            })
        ),
        "got {outcome:?}"
    );
}

#[test]
fn an_unanswered_probe_carrying_a_finding_is_refused() {
    // The other direction: a result under "not started" means someone changed the verdict back and left the
    // finding, or wrote a conclusion nobody measured.

    // Arrange
    let source = register(
        "## P01 A title\n\n\
         - **Verdict:** not started\n\
         - **Milestone:** M1\n\
         - **Question:** a question.\n\
         - **Method:** a method.\n\
         - **Decides:** a decision.\n\
         - **Result:** it worked.\n",
    );

    // Act
    let outcome = Register::parse(&source);

    // Assert
    assert!(
        matches!(outcome, Err(ParseError::ResultWithoutFinding { .. })),
        "got {outcome:?}"
    );
}

#[test]
fn a_measurement_keeps_every_paragraph_of_its_finding() {
    // Arrange
    let source = register(
        "## P01 A title\n\n\
         - **Verdict:** measured\n\
         - **Milestone:** M1\n\
         - **Question:** a question.\n\
         - **Method:** a method.\n\
         - **Decides:** a decision.\n\
         - **Result:** the first paragraph.\n\n\
         The second paragraph, with the command that produced it.\n",
    );

    // Act
    let parsed = Register::parse(&source).unwrap();

    // Assert
    let result = parsed.probes().first().unwrap().result.clone().unwrap();
    assert!(result.contains("the first paragraph."), "got {result:?}");
    assert!(result.contains("The second paragraph"), "got {result:?}");
}

#[test]
fn an_empty_register_is_refused() {
    // A parser that returned an empty register here would let every check downstream pass by measuring
    // nothing.

    // Act
    let outcome = Register::parse("# Evidence register\n\nNothing here yet.\n");

    // Assert
    assert!(matches!(outcome, Err(ParseError::Empty)), "got {outcome:?}");
}

#[test]
fn an_identifier_is_recognised_only_as_a_whole_token() {
    // Arrange
    let text = "Probes P01, P02 and P15 are recorded. MP03 is not an identifier, and neither is P4X or P1.";

    // Act
    let found = probe_mentions(text);

    // Assert
    let rendered: Vec<String> = found.iter().map(ToString::to_string).collect();
    assert_eq!(rendered, vec!["P01", "P02", "P15"]);
}

#[test]
fn a_probe_its_milestone_never_mentions_is_reported() {
    // The register and the roadmap are two halves of one contract. A probe nobody can reach from the
    // roadmap is a question nobody will answer.

    // Arrange
    let register = Register::parse(&register(&entry("P01", "M1", "not started"))).unwrap();
    let roadmap = Roadmap::parse("# Roadmap\n\n## M1: A milestone\n\nNothing here names a probe.\n").unwrap();

    // Act
    let found = register.reconcile(&roadmap);

    // Assert
    assert!(
        matches!(found.as_slice(), [Discrepancy::NotMentionedInItsMilestone { .. }]),
        "got {found:?}"
    );
}

#[test]
fn a_probe_the_roadmap_invents_is_reported() {
    // Arrange
    let register = Register::parse(&register(&entry("P01", "M1", "not started"))).unwrap();
    let roadmap =
        Roadmap::parse("# Roadmap\n\n## M1: A milestone\n\nProbes P01 and P44 are recorded.\n").unwrap();

    // Act
    let found = register.reconcile(&roadmap);

    // Assert
    assert!(
        matches!(found.as_slice(), [Discrepancy::MentionedButNotRegistered { .. }]),
        "got {found:?}"
    );
}

#[test]
fn a_probe_assigned_to_a_milestone_that_does_not_exist_is_reported() {
    // Arrange
    let register = Register::parse(&register(&entry("P01", "M9", "not started"))).unwrap();
    let roadmap = Roadmap::parse("# Roadmap\n\n## M1: A milestone\n\nProbe P01.\n").unwrap();

    // Act
    let found = register.reconcile(&roadmap);

    // Assert
    assert!(
        found
            .iter()
            .any(|discrepancy| matches!(discrepancy, Discrepancy::UnknownMilestone { .. })),
        "got {found:?}"
    );
}

#[test]
fn a_register_and_a_roadmap_that_agree_report_nothing() {
    // Arrange
    let register = Register::parse(&register(&entry("P01", "M1", "not started"))).unwrap();
    let roadmap = Roadmap::parse("# Roadmap\n\n## M1: A milestone\n\nProbe P01 is recorded.\n").unwrap();

    // Act
    let found = register.reconcile(&roadmap);

    // Assert
    assert_eq!(found, Vec::new());
}
