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
| G01 | No tracked file carries an em dash or an en dash. The characters are built from their code points, so the test does not contain what it forbids | [ADR-0012](adr/0012-commits-are-signed-and-carry-no-trailers.md) |
| G02 | Every workflow step references an action by a 40-character commit, and keeps its version in a trailing comment | [`SECURITY.md`](../SECURITY.md) |
| G03 | Every directory holding non-Markdown source is covered by a workflow path filter, with the directories discovered from git rather than from a list | [ADR-0001](adr/0001-a-single-repository.md) |
| G04 | Every relative link in a tracked Markdown file resolves, and the decision index lists every record | [ADR-0011](adr/0011-documentation-is-rendered-from-the-repository.md) |
| G05 | No tracked file carries an absolute path from a developer machine, a network address, or private key material | [`CONTRIBUTING.md`](../CONTRIBUTING.md) |
| G06 | The repository root holds only the files it declares, and every declared file exists | [ADR-0001](adr/0001-a-single-repository.md) |
| G07 | Exactly one toolchain pin exists, it is patch-exact, no pipeline restates a version, and the workspace minimum is satisfied by the pin | [`docs/getting-started.md`](getting-started.md) |
| G08 | The licence identifier agrees across the licence file, the workspace manifest and the citation metadata; the licence text is unaltered; the notice file exists; and the dependency policy admits no reciprocal licence | [ADR-0003](adr/0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md) |
| G09 | Markdown prose wraps at 110 columns, with tables, code blocks and unbreakable tokens excepted | [ADR-0011](adr/0011-documentation-is-rendered-from-the-repository.md) |
| G10 | A directory whose only tracked file is a README says which milestone fills it, and a directory that says so holds nothing else | [ADR-0001](adr/0001-a-single-repository.md) |
| G11 | Unsafe code is forbidden workspace-wide and appears nowhere; the panicking constructs are denied outside tests; every crate inherits the workspace lints | [ADR-0002](adr/0002-one-rust-core-and-thin-shells.md) |
| G20 | Every probe in the register names a milestone the roadmap has and is named by that milestone, and the roadmap mentions no probe the register does not hold | [`docs/evidence.md`](evidence.md) |
| G52 | The ledger accounts for every identifier once, the enforced tables list exactly the guarantees the suite holds, the README's counts match, and the documented gate sequence is the one the script runs | this document |

Two of these arrived earlier than the plan scheduled them. **G20** came to M0 because the evidence register
was worth reading as data from the first day. **G52** came to M0 because it caught a wrong number in the
README on the day the README was written, which is exactly the failure it exists to prevent.

## Committed, with the milestone that will enforce each

### M1, inspection and fingerprints

| Id | Guarantee |
|---|---|
| G12 | Parsing then reserialising without an edit reproduces the input byte for byte, including an unknown child inside a sample description entry |
| G13 | No code path constructs a typed user-data structure, and an unknown user-data child survives every public entry point |
| G14 | The production fingerprint equals the published reference script over the whole corpus, and the secondary digest equals the specified third-party recipe at a pinned tool version |
| G15 | The parser never panics on arbitrary input, fuzzed nightly and smoke-fuzzed on every pull request |
| G16 | No user-visible string in any catalogue found on disk asserts a verdict or uses internal jargon, matched as whole words in a verdict role, per language |
| G17 | Every time value in a manifest carries a source from a closed set, and every sentence about time names its clock |
| G18 | Verification runs against real files, and a single flipped byte in a sample or in the vendor box produces a mismatch and nothing else |
| G19 | Inspecting a recording in header-only mode reads under 256 KiB and never touches the media payload |

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

### M5, desktop

| Id | Guarantee |
|---|---|
| G37 | Every release artefact carries an attestation that the command printed in the README accepts |
| G38 | Committed generated bindings match the core they were generated from |
| G39 | The desktop shells never write to a source, watched with a file monitor over a scripted session |
| G40 | A stream-copy export from each shell produces fingerprints equal to the command line's |
| G41 | The sandboxed package declares its codec extension and reports a clear unavailable state when it is masked |

### M6, telemetry and masking

| Id | Guarantee |
|---|---|
| G44 | The telemetry parser never fails an export; an unknown layout degrades to bytes preserved and the export carries the bytes |
| G45 | A share action is reachable only after masking, or after an explicit acknowledgement |

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
