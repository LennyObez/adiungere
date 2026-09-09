# Changelog

Notable changes to adiungere. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/);
versioning follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Tags are prefixed by artefact,
because one repository publishes several things.

## [Unreleased]

### Added

- Repository foundation: layout, licence, contribution, conduct and security policies, and the publication
  rule that a tracked file states what the code guarantees rather than what once failed.
- Fourteen decision records for the decisions the product rests on: one repository; one Rust core with thin
  shells and each platform's own media engine; Apache-2.0 with a notice and no contributor agreement; boxes
  as opaque byte ranges unless a value is needed; a product that reports facts and never returns a verdict;
  every export through the core; one desktop answer per platform; a website that runs in the browser with a
  service that sees only digests; sign last with a single key custodian; English as the source language with
  French shipping alongside the first product; documentation rendered from the repository; signed commits
  with no trailers; no update check in any application; and the verification surface named for a place
  rather than for a conclusion.
- Roadmap of eleven milestones, each ending in a named demonstration, with the first real product at M4.
- Evidence register of 36 probes, each with a question, a method, the decision it settles and one of four
  verdicts, where `unavailable` and `not started` are never read as a pass.
- Command line reading that register as data: a listing that filters by verdict and milestone, a detail view,
  and a reconciliation that fails when the register and the roadmap disagree. Its parser refuses a register
  whose shape has drifted, refuses an entry out of order, and refuses a measurement with no finding written
  underneath.
- Rust workspace with the toolchain pinned patch-exact in one file at the root, unsafe code forbidden
  workspace-wide, the panicking constructs denied outside tests, exact dependency versions with the lock file
  committed, and a supply-chain policy admitting no reciprocal licence, no unknown registry and no git
  source.
- The guarantee suite, each member watched failing against a deliberate violation before being trusted, and
  each scanning member carrying a detection test and a reach test so it cannot pass by reading nothing. Two of
  them reconcile the documentation with the repository, so a count written in a README cannot quietly stop
  being true.
- Continuous integration with a repository-wide workflow that carries no path filter, a workflow per
  artefact, a check that discovers directories from git and fails when one holds source no pipeline builds,
  and a check that every relative documentation link resolves.
- Declared label set applied from the repository on change and weekly, dependency updates for the workspace
  and the pipelines, issue forms for a bug, a feature and a probe, and a pull request template whose
  checklist names the properties this product cannot afford to lose.
- Static placeholder page for the project's domain, deliberately carrying no brand identity, since the
  identity is designed at M4 and inventing one now would be a decision taken by default.
- Ledger of every guarantee, enforced and committed, against the milestone that will enforce each.
