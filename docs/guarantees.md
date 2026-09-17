# Guarantees

A **guarantee** is a property of the repository or the product that a test refuses to let anyone break. It is
not a convention, a comment or an intention. It fails on its own, for everyone, in six months.

Two rules govern this ledger.

**A guarantee is added in the milestone that adds the behaviour it protects, never before.** A test asserting
a property of code that does not exist is a placeholder reporting success, which is worse than an admitted
gap. Everything below marked with a later milestone is a commitment, and the milestone is where to hold it
to account.

**A guarantee is watched failing before it is trusted.** Introduce the violation, run the suite, see it fail
and name the offending file, then revert. A guarantee nobody has seen fail is a guarantee nobody has
verified. Where the guarantee is a scan, it carries two companions: a detection test proving the patterns
match what they describe, and a reach test proving the scan read real files. Both are present for every
scanning guarantee enforced today.

The identifiers are stable. They are not an order of delivery, which is why the enforced set below is not a
contiguous range.

## Enforced today

| Id | Guarantee | Enforces |
|---|---|---|
| G01 | No tracked file carries an em dash, an en dash, a look-alike, or the HTML entity a renderer turns into one. The characters are written as escapes, so the test does not contain what it forbids | [ADR-0012](adr/0012-commits-are-signed-and-carry-no-trailers.md) |
| G02 | Every action a workflow or a composite action runs is referenced by a 40-character commit or an image digest, under every spelling of the key, and keeps a version in its trailing comment | [`SECURITY.md`](../SECURITY.md) |
| G03 | Every directory that directly holds non-Markdown source is built by a workflow whose `paths` list names it, or an ancestor, with a glob; a sibling's glob does not count, an ignore list does not count, and the directories are discovered from git | [ADR-0001](adr/0001-a-single-repository.md) |
| G04 | Every relative link in a tracked Markdown file, inline, reference-style or raw HTML, resolves inside the repository to a file that exists and, when it names an anchor, to a heading that exists; and the decision index links every record | [ADR-0011](adr/0011-documentation-is-rendered-from-the-repository.md) |
| G05 | No tracked file carries an absolute path from a developer machine, a network address of either family, credential material, or a private key, in any prose form; and a tracked file not declared binary that does not decode as text refuses the whole scan rather than escaping it | [`CONTRIBUTING.md`](../CONTRIBUTING.md) |
| G06 | The repository root holds only the files and directories it declares, each with a stated reason, and every declared entry is tracked | [ADR-0001](adr/0001-a-single-repository.md) |
| G07 | Exactly one toolchain pin exists and it is patch-exact; the workspace minimum, the lint minimum and every crate agree with it; no pipeline restates, overrides or selects a compiler by any of the means the toolchain manager honours; no script writes a compiler version in its own text; and the dated nightly the fuzzer uses is pinned to the day in the tool versions file | [`docs/getting-started.md`](getting-started.md) |
| G08 | The licence identifier agrees across the licence file, the workspace and every crate manifest, and the citation metadata; the licence text matches its canonical publication to the byte; the notice file exists; and the dependency policy and the dependency review admit exactly the accepted set, with no exception and no clarification | [ADR-0003](adr/0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md) |
| G09 | Markdown prose, under every spelling of the extension, wraps at 110 characters, with tables, fenced blocks, whole-line comments and unbreakable tokens excepted | [ADR-0011](adr/0011-documentation-is-rendered-from-the-repository.md) |
| G10 | A directory whose only tracked file is a README, under any casing or Markdown extension, says which existing milestone fills it, and a directory that says so holds nothing else | [ADR-0001](adr/0001-a-single-repository.md) |
| G11 | Unsafe code is forbidden workspace-wide and appears in no tracked Rust file, read as code rather than as prose about code; the panicking constructs are denied outside tests; every crate inherits the workspace lints or is named as restating them; and no source attribute switches any of these off | [ADR-0002](adr/0002-one-rust-core-and-thin-shells.md) |
| G12 | Reading a recording and re-emitting its ranges reproduces the input byte for byte on every corpus recording, unknown boxes included and an unknown child inside a sample entry included: the ranges tile the file with no gap and no overlap | [ADR-0004](adr/0004-boxes-are-opaque-byte-ranges.md) |
| G13 | No crate declares a typed user-data box, and a vendor child of the user-data box is recoverable byte for byte after reading and is recorded in the manifest by the digest of its own bytes | [ADR-0004](adr/0004-boxes-are-opaque-byte-ranges.md) |
| G14 | The track fingerprint and the elementary stream digest equal what the reference script computes on every track of the corpus, and the elementary stream digest equals what the pinned media tool writes; a missing interpreter or tool is a failure, never a skip | [`docs/integrity.md`](integrity.md) |
| G15 | A fuzz target exists for every parser and the nightly pipeline runs each one; the workspace denies every panicking construct outside tests; and a deterministic mutation pass over the corpus runs on every pull request | [ADR-0002](adr/0002-one-rust-core-and-thin-shells.md) |
| G16 | No string in any wording catalogue, in any language found on disk, contains a word from that language's forbidden list, matched as a whole word without regard to case; every language carries the same keys as English | [ADR-0005](adr/0005-the-product-never-returns-a-verdict.md) |
| G17 | The published schema closes the set of clocks a time can be read from and requires one on every time; every time a manifest carries has a clock and a note; every sentence about a time says "no later than" and names its clock | [ADR-0005](adr/0005-the-product-never-returns-a-verdict.md) |
| G18 | One flipped byte in a sample is that track's finding and the whole file's and nothing else; one flipped byte in a vendor box, under user data or at the top level, is that box's finding and the whole file's and nothing else | [`docs/integrity.md`](integrity.md) |
| G19 | Inspecting a recording's structure reads fewer than 256 kibibytes and never touches the media data box, measured through a counting source on every corpus recording, with the movie box first and last | [ADR-0004](adr/0004-boxes-are-opaque-byte-ranges.md) |
| G20 | Every probe in the register names a milestone the roadmap has and is named in that milestone's own prose, and the roadmap mentions no probe the register does not hold; a register that has drifted in shape is refused rather than read partially | [`docs/evidence.md`](evidence.md) |
| G21 | Every export mode of every corpus recording, written and read back, carries every user-data child and every unknown top-level box byte for byte, places the movie box before the media data, and gives every kept track the fingerprint and the elementary stream digest it had in the source; a vendor box short of one byte is seen to fail the comparison | [ADR-0006](adr/0006-every-export-goes-through-the-core.md) |
| G22 | The digests of the three export modes of the reference-like recording are pinned in the reader crate's own tests and are what the writer produces; the platform matrix runs that suite on Linux, macOS on both architectures and Windows, and excludes nothing that carries a pin. The WebAssembly writer joins the comparison at M4 | [ADR-0006](adr/0006-every-export-goes-through-the-core.md) |
| G23 | The pinned media tool counts, for every export mode of every corpus recording, the same packets per stream as the product wrote samples, and decodes every stream without a word on its error stream; a truncated export is seen to draw a complaint | [`docs/integrity.md`](integrity.md) |
| G24 | No source file of the reading crates opens a file for writing, moves, copies, truncates or re-dates one, the scan cache excepted for its own file; and every command of the product, run on a recording, leaves that recording's digest and modification time exactly as they were. Each shell adds its own scripted session, from M4 to M8 | [ADR-0006](adr/0006-every-export-goes-through-the-core.md) |
| G25 | No crate of the resolved dependency graph is an encoder, a media framework that bundles one, or a binding named after one; and the release command line, built with its symbol table kept, carries no defined or imported symbol from the forbidden list, read with the toolchain's symbol reader after a fixture carrying one such name has been refused first | [ADR-0006](adr/0006-every-export-goes-through-the-core.md) |
| G26 | Every sign the derivative detector declares is raised by at least one corpus recording scanned as one population, the reference-like recording raises none, each sign is worded from the catalogue in the command line's description, and a container whose name fits no grammar is described as unplaced and never as clean | [ADR-0005](adr/0005-the-product-never-returns-a-verdict.md) |
| G27 | Signing is the last step: an export the product signs is valid to the pinned validator, which is not this product; the same file remuxed with every sample kept and the store kept, as a rewriter that does not know the standard produces it, is invalid on the hash of the container; signing it again makes it valid; and the product's own reader reports the same three states | [ADR-0009](adr/0009-sign-last-and-a-single-key-custody.md) |
| G28 | Every corpus recording that receives credentials beside it keeps its bytes and its modification time, the credentials are found under the recording's stem by the pinned validator, which finds the binding intact, and one changed byte of the recording is seen to make it refuse the pair | [ADR-0009](adr/0009-sign-last-and-a-single-key-custody.md) |
| G29 | For an export signed through the command line, every track digest and every source digest is the same in the manifest beside the file, in the signed facts the validator reads out of the file and in the report rendered for a person, and the action recorded follows from the export class alone: a stream copy is a repackaging, a re-encoding a transcoding, a recording as it is an opening | [`docs/integrity.md`](integrity.md) |
| G30 | The signer's common name the product shows is the one the pinned validator reads from the certificate; a chain that leads to no trust list is shown as exactly that and never as more; a broken binding is shown as invalid with the validator's code; and the product's state type has one variant per validator state, so no fourth word exists | [ADR-0015](adr/0015-what-a-relayed-signature-proves.md) |
| G52 | The ledger accounts for every identifier exactly once; the enforced tables list exactly the guarantees the suite holds and each of those holds a test; the README's counts of guarantees and probes and the changelog's count of decision records match the repository; and the gate sequence the documentation lists is the one the script runs, in both directions and in order | this document |
| G54 | Nothing the ignore file refuses is tracked, under any casing, with the rules read from the ignore file itself; and no tracked file exceeds one mebibyte, because source is never that large and a recording or a build artefact is | [`CONTRIBUTING.md`](../CONTRIBUTING.md) |

Two of these arrived earlier than the plan scheduled them. **G20** came to M0 because the evidence register
was worth reading as data from the first day. **G52** came to M0 because a count in a document is wrong the
moment nobody checks it, and the first day is when the counts are written. **G54** was not in the plan: the
platform's push rules would refuse a recording or a key at the door for a repository under an organisation,
and this repository is not one, so the suite holds the door instead.

## Committed, with the milestone that will enforce each

Three of the enforced guarantees grow with the surfaces: **G22** compares the WebAssembly writer from M4,
**G24** adds a scripted session for each shell from M4 to M8, and **G30** holds each surface's rendering of
the signer and the state from M4 to M9, when a credential from a listed authority gives the third state
something to show. None is listed again below, because the test that holds each already exists and the
extension is a row in that test, not a new guarantee.

### M4, website and shared player

| Id | Guarantee |
|---|---|
| G31 | Every text-on-surface token pair meets the contrast floor, and every integrity state is distinguishable without colour |
| G32 | The web bundle constructs no shared memory and uses no atomics, so the site needs no cross-origin isolation |
| G33 | The service refuses any body over the cap, accepts no multipart field, and logs no body |
| G34 | No request leaves the origin during scan, playback or export, apart from the digest calls a person triggers |
| G35 | The integrity label derives from the export class, and a composition never carries the sample-identical statement |
| G36 | Every version declaration agrees, whichever script wrote it |
| G42 | The pixel-exact label is emitted only after every frame has been compared |
| G43 | A composition carries the source telemetry bytes and the source fingerprints, and its digest differs from every source |

### M5, desktop, with G53 extended to each later surface

| Id | Guarantee |
|---|---|
| G37 | Every release artefact carries an attestation that the command printed in the README accepts |
| G38 | Committed generated bindings match the core they were generated from |
| G39 | The desktop shells never write to a source, watched with a file monitor over a scripted session |
| G40 | A stream-copy export from each shell produces fingerprints equal to the command line's |
| G41 | The sandboxed package declares its codec extension and reports a clear unavailable state when it is masked |
| G53 | An installed application makes no network request during scan, playback, export or verification, apart from the signing and time-stamping calls a person triggers, watched over a scripted session on each surface as it ships |

### M6, telemetry and masking

| Id | Guarantee |
|---|---|
| G44 | The telemetry parser never fails an export; an unknown layout degrades to bytes preserved and the export carries the bytes |
| G45 | A share action is reachable only after masking, or after an explicit acknowledgement that the file is unmasked; the choice is recorded in the manifest and never defaulted |

### M7, Android

| Id | Guarantee |
|---|---|
| G46 | Video track selection is always an explicit override, never a default |
| G47 | The store-free flavour resolves no proprietary service in its dependency graph |
| G48 | Android never opens a source for writing, and writes its outputs through the pending mechanism rather than a path column |

### M8, Apple

| Id | Guarantee |
|---|---|
| G49 | The macOS bundle carries the photo library entitlement and its usage string, and the application asserts both at launch |
| G50 | The Apple shell never mutates a photo library original |

### M9

| Id | Guarantee |
|---|---|
| G51 | The production signer chain validates against the published trust anchors, checked nightly |

## Where they live

The suite is `core/crates/adiungere-guarantees`, one file per guarantee, named after it. A guarantee that
scans the repository asks git for the file list through an argument vector, never through a shell, so
nothing here builds a command out of a string.
