# Testing

A suite is worth exactly what it would catch. This document states the method, the structure, what is
refused, and the gates that must be green before anything merges.

## Method: the test fails first

A test is written before the code that satisfies it, and it is **run in the failing state** before that code
exists. Watching it fail is the only evidence that it tests something. A test written afterwards against code
that already passes proves that the code does what it does.

A defect follows the same order: a test that reproduces it, run and seen to fail, then the fix, then the test
passing. A fix with no failing reproduction is a claim.

## Structure: arrange, act, assert

Every test has three visible parts, in order, separated by blank lines. Arrange builds the world, act performs
exactly one action, assert checks the observable result of that action.

No branching and no loops in the body of a test. A test that needs a condition is two tests. A test that needs
a loop over cases is a table-driven test with the cases as data.

One behaviour per test, and the name says which behaviour in plain words.

## Refused: the tautological test

A test that reproduces the implementation in order to compare it with itself measures nothing while reporting
success. Three forms, all rejected in review:

- **Asserting on a double the test just configured.** Telling a stub to return a value and then asserting that
  the value came back tests the stub.
- **Recomputing the expectation with the code under test.** The expected value is written literally, or it
  comes from an independent source. A value too tedious to write by hand is a signal that the unit is too big.
- **Asserting that an accessor accesses.** Plain accessors are exercised by the tests of the behaviour that
  uses them.

The rule underneath all three: a test asserts on behaviour observable from outside the unit, never on the
steps the unit took inside itself.

## What this product needs beyond ordinary tests

This is a file-format product used for evidence, so three kinds of check carry more weight here than the unit
suite does.

**Golden tests.** A known input, a known plan, and an expected output compared byte for byte. They are the
only honest way to test a writer, because "it plays" is not a specification.

**Property tests.** The invariants are strong enough to state: parsing then reserialising without an edit
reproduces the input exactly; an extraction preserves the vendor bytes; two runs on two operating systems
produce the same bytes. Generated input finds the cases nobody writes by hand.

**Independent oracles.** A parser we wrote cannot confirm a file we wrote. Another parser reading the same
sample counts, and a third-party command reproducing the same digest, is what makes the claim checkable by a
stranger. Oracle jobs are pinned to exact tool versions from M1, because an unpinned oracle produces a false
green or a false red and both cost a day.

Each arrives with the milestone that gives it something to measure. A gate with nothing to check reports
success and teaches everyone to trust a green that means nothing.

| Kind of check | Arrives |
|---|---|
| Golden tests per export mode | M2 |
| Property tests over the synthetic corpus | M1 |
| Independent parser and third-party digest oracles | M1, M2 |
| Fuzzing of the box reader, the sample iterator and both parsers | M1 |
| Mutation testing and undefined-behaviour checking | M1 |
| Byte-identical output across operating systems and WebAssembly | M2, M4 |
| Browser tests on three engines with a network assertion | M4 |

## Guarantees expressed as tests

A **guarantee** is a property of the whole repository or the whole product that a test refuses to let anyone
break. They live in one crate so a reader finds them in one place, and each names the decision record it
enforces.

**A guarantee is added in the milestone that adds the behaviour it protects, never before.** A test asserting
a property of code that does not exist is a placeholder reporting success, which is worse than an admitted
gap. The README states which guarantees hold today and which milestone enforces each of the rest.

### Writing one

A guarantee test differs from an ordinary test in one respect: **it must be seen to fail on a real
violation before it is trusted.** Introduce the violation, run the suite, watch it fail and name the offending
file, then revert. A guarantee nobody has seen fail is a guarantee nobody has verified.

Where the guarantee is a scan, it needs two companions:

- **A detection test**, proving the patterns match what they describe. A pattern that matches nothing makes
  the guarantee silently vacuous.
- **A reach test**, proving the scan read real files. A scan whose file discovery is wrong matches nothing and
  reports success.

Both are present for every scanning guarantee in the suite today.

## Gate integrity

A gate that reports a false green is worse than no gate. These are not style preferences.

- **Never pipe before reading the exit code.** A piped command reports the last stage's status. Write to a
  file, read the status, then inspect the file.
- **Never run a gate with a partial configuration.** A scoped run answers a different question, and its green
  is not the gate's green.
- **Never generalise from a slice to the whole.**
- **Never truncate a measurement.** No head, no tail, no result limit on a search whose purpose is to count or
  enumerate. A truncated enumeration is a different answer that looks like the real one.
- **Never run two heavy measurements at once.** Contention reads as a result.
- **Never silence a finding.** No allow attribute, no exclusion, no ignore entry added to make a gate pass.
  Fix the cause. The only ignores this repository permits are advisory entries in the dependency policy, each
  with a written reason and a date, and a lint allowed at a single site with a comment saying why.

## The gate sequence

Run in this order; each must be green before the next means anything. `scripts/gate.sh` runs the whole
sequence, writes each step's output to its own file, reads each exit code before moving on, and stops at the
first red.

| # | Step | Command today |
|---|---|---|
| 1 | Formatting | `cargo fmt --all --check` |
| 2 | Lints, warnings refused | `cargo clippy --workspace --all-targets --all-features -- -D warnings` |
| 3 | Unit tests | `cargo test --workspace --lib` |
| 4 | Integration and guarantee tests | `cargo test --workspace --tests` |
| 5 | Documentation tests | `cargo test --workspace --doc` |
| 6 | The lock file matches the manifests | `cargo metadata --locked` |
| 7 | Evidence register agrees with the roadmap | `cargo run -p adiungere-cli -- probes check` |
| 8 | The published site loads nothing from another host, and its security contact is current | `scripts/check-site.sh` |
| 9 | Shell scripts | `shellcheck` over `scripts/` |
| 10 | Advisories, licences, sources and bans | `cargo deny check` |
| 11 | Golden and property suites | M1, with the first fixture corpus |
| 12 | Mutation testing, fuzzing, undefined-behaviour checking | M1, nightly |
| 13 | Cross-platform and WebAssembly build matrix | M1 |

A guarantee holds that this table and the script cannot drift apart: every step the script runs has to be
described here, which is **G52**. A gate the documentation describes and the script does not run is a step
nobody performs while everyone believes it happens.

Continuous integration runs each step as its own step, never chained behind a single shell line, so a failure
names itself.

## What "done" means

A change is done when the full sequence has been run, not a subset and not a scoped run, and the command that
proves it can be quoted. A result announced without the command that produced it is not a result.
