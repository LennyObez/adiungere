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
| G52 | The ledger accounts for every identifier exactly once; the enforced tables list exactly the guarantees the suite holds and each of those holds a test; the README's counts of guarantees and probes and the changelog's count of decision records match the repository; and the gate sequence the documentation lists is the one the script runs, in both directions and in order | this document |
| G54 | Nothing the ignore file refuses is tracked, under any casing, with the rules read from the ignore file itself; and no tracked file exceeds one mebibyte, because source is never that large and a recording or a build artefact is | [`CONTRIBUTING.md`](../CONTRIBUTING.md) |

Two of these arrived earlier than the plan scheduled them. **G20** came to M0 because the evidence register
was worth reading as data from the first day. **G52** came to M0 because a count in a document is wrong the
moment nobody checks it, and the first day is when the counts are written. **G54** was not in the plan: the
platform's push rules would refuse a recording or a key at the door for a repository under an organisation,
and this repository is not one, so the suite holds the door instead.

## Committed, with the milestone that will enforce each

### M2, lossless extraction

| Id | Guarantee |
|---|---|
| G21 | Every lossless export preserves the vendor telemetry box and the unknown top-level box byte for byte, writes the movie box first, and produces fingerprints equal to the source |
| G22 | The same input and the same plan produce identical bytes on Linux, macOS and Windows, and from M4 in WebAssembly |
| G23 | The rebuilt sample tables are coherent and an independent parser counts the same samples |
| G24 | No surface ever opens a source for writing, proved by a reader type with no write capability and by digests taken before and after a scripted session |
| G25 | The stream-copy path links no encoder, proved by the dependency tree and by inspecting the released binary for encoder symbols |
| G26 | The derivative detector raises its banner on every signal in the fixture corpus |

### M3, provenance

| Id | Guarantee |
|---|---|
| G27 | Signing is the last step: a rewrite applied after signing invalidates the binding, and re-signing restores it |
| G28 | An original that receives an external manifest is byte-identical to the original |
| G29 | The report, the provenance manifest and the integrity manifest agree on every digest and every action |
| G30 | The displayed signer identity equals the certificate subject, and the trust state equals the validator's result |

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
