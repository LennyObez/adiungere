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
- Evidence register of 40 probes, each with a question, a method, the decision it settles and one of four
  verdicts, where `unavailable` and `not started` are never read as a pass.
- Command line reading that register as data: a listing that filters by verdict and milestone, a detail view,
  and a reconciliation that fails when the register and the roadmap disagree. Its parser refuses a register
  whose shape has drifted, refuses an entry out of order, and refuses a measurement with no finding written
  underneath.
- Rust workspace with the toolchain pinned patch-exact in one file at the root, unsafe code forbidden
  workspace-wide, the panicking constructs denied outside tests, exact dependency versions with the lock file
  committed, and a supply-chain policy admitting thirteen named licences and nothing copyleft beyond the
  file level, no unknown registry and no git source, applied to the fuzzing project as well as the
  workspace.
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
- Guarantee that nothing the ignore file refuses is tracked and that no tracked file is larger than source
  ever is, because the platform cannot hold that door for a repository owned by a person.
- Box reader written without I/O, so that one state machine serves a file, a slice and a browser's segmented
  reads with the same requests in the same order. Every box is an opaque byte range that tiles its parent;
  the few typed views the product needs are read from those ranges and never resynthesised. The user-data
  box and its children, an unknown box inside a sample entry, and an unknown box at the top level all
  survive as bytes. A file that ends in pre-allocated space, or in a media data box a power loss cut short,
  is read with the clipped tail described as such. Every read is bounded before a byte is allocated.
- Sample iterator in decode order over both chunk offset widths, both size table forms, sync and
  composition tables, with every table cross-checked and every inconsistency named after its track.
- Fingerprints: the track fingerprint over the stored samples and the decoder configuration; the
  elementary stream digest that reproduces the common media tool's output byte for byte; the whole-file
  digest; the structural fingerprint of the writer. The integrity specification publishes each rule with a
  reference script in the interpreter's standard library and the third-party commands that reproduce every
  number.
- Manifest format with unknown fields refused, a published JSON schema regenerated by a test, canonical
  text, every time carrying its clock from a closed set and a note on what that clock is worth, and a
  comparison that reports a finding per subject and never a verdict.
- Scanner: three naming grammars described by shape and never by brand, a probe that reads the movie box
  and nothing else, pairing of one-file-per-camera recordings by their stem, the signs that another program
  wrote a file, a cache keyed by size and modification time, and an extension filter so a photo library is
  not opened file by file.
- Command line: `inspect`, `fingerprint`, `detect`, `verify` and `report`, each with prose for a person and
  one JSON document for a pipeline, every sentence drawn from a wording catalogue that a guarantee reads.
- Public synthetic fixture: pre-encoded frames of a synthetic pattern, muxed by the repository into ten
  recordings that carry every property the reference recording has, and the private recording pinned by
  digest and reproduced only in the nightly pipeline.
- External oracles pinned in one file the pipelines read: the interpreter, the media tool by build and
  digest, the dated nightly, the fuzzer and the mutation tester. A browser reader and an Apple reader as
  independent oracles of the fingerprint.
- Fuzzing of every parser on the dated nightly, mutation testing and the undefined-behaviour interpreter,
  a mutation smoke on every pull request, and a core matrix over Linux, macOS, Windows, musl and
  WebAssembly.
- Eight further guarantees enforced, each watched failing: the reader round-trips every byte; no code path
  types the user-data box; the fingerprints equal their references; the reader never panics; no wording
  reads as a verdict; every time names its clock; one flipped byte is one finding; an inspection is
  bounded.
- Remuxer over the box reader: a subset of tracks from one or two recordings written to a new container
  in two passes, the movie box first, chunks interleaved by decode time in a fixed order, every coded
  sample copied byte for byte, every user-data child and every unknown top-level box carried across as
  the bytes they were, the sample description entries and the edit lists untouched, and only the chunk
  offset tables rebuilt, widened to 64 bits when the media reaches past four gibibytes. Two inputs are
  joined into one file with the tracks renumbered. Progress is reported and a cancellation removes
  everything written.
- Command line `export`: one camera, both, or the tracks named, from one recording or from the two files
  of a recorder that writes one per camera; a manifest written beside the output naming each source, its
  digest and the fingerprint of every track; the output read back and refused, and removed, when a track
  does not carry its source's fingerprint; a progress line and a clean stop on interruption.
- Manifest operation for an export, with the export class and the masking choice recorded permanently:
  an extraction, a two-track archive, a lossy rendition or a rendition verified lossless, and no masking,
  faces and plates, or faces, plates and the burned-in strip.
- Golden digests of the three export modes pinned in the reader crate's tests and run on every platform
  of the matrix; property tests over every export mode of every corpus recording; the pinned media tool
  counting packets and decoding every export as an independent parser.
- Encoder symbol check over the release command line with the toolchain's own symbol reader, refusing a
  fixture that carries one forbidden name before it judges the product, run by the gate and the pipeline.
- The derivative detector assesses a container whose name fits no grammar with the signs any container
  shows, and the command line describes such a file as unplaced rather than as not recognised.
- Six further guarantees enforced, each watched failing: every export preserves the recorder; the same
  input writes the same bytes everywhere; an independent parser reads the same samples; no surface opens a
  source for writing; the stream copy links no encoder; the detector raises every sign on the corpus.
- Probe P40 for the half of the extraction demonstration no pipeline can run: the rear-only export of
  the reference recording played in full in the gallery application of Windows and on an iPhone; an
  Android gallery joins the probe when a device is at hand.
- Track references follow the tracks they name: an export rewrites them to the output's identifiers and
  leaves out those naming a track the output does not hold, and says how many it left out. An unknown
  top-level box larger than the reader holds is copied from the source in pieces rather than refused.
  An export refuses, before writing a byte, any output, partial or manifest path that names one of its
  sources under any spelling. The output read back is held to its sources' decoder configuration digest
  as well as their sample digest.
- The corpus gains a recording whose rear track references the front and the audio tracks.

[Unreleased]: https://github.com/LennyObez/adiungere/commits/main
