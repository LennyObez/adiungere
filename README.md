# adiungere

A dashcam with two cameras writes **one** file holding both of them. Galleries and players show the first
camera and nothing else. adiungere finds those recordings, plays either camera or both together, exports
each one without re-encoding it, and keeps the vendor telemetry that every other route throws away.

The name is Latin: to join, to attach to. Two views of one moment, joined rather than separated.

**Status: early construction.** The command line inspects a recording, fingerprints every track, finds
recordings in a library, exports either camera or both without re-encoding a sample and with the recorder's
boxes carried across byte for byte, joins the two files of a recorder that writes one per camera, signs an
export with Content Credentials embedded or a recording with credentials beside it, asks an authority for
a time stamp, and compares a manifest and its credentials with a file; nothing plays yet, and every
signature is made with a credential on no trust list until one from a listed authority is enrolled. What
follows
describes the product being built, and [Guarantees](#guarantees) separates the properties a test enforces
today from those committed to, each against the milestone that will enforce it.
[`docs/roadmap.md`](docs/roadmap.md) holds the plan.

---

## The problem

A two-camera dashcam does not write two files. It writes one file with two video tracks and one audio track,
and every consumer gallery shows the first track, because that is what its demultiplexer selects. The rear
camera is in the file. Nothing offers to show it.

Reaching it today means a command line and an external tool. That route works, and it silently throws away
the vendor telemetry box: on the reference recording measured in the evidence register (probe P37), roughly
thirty kilobytes holding the camera identity, one satellite fix per second and about twenty motion records
per second. It also rewrites the container in a way that stops the file playing progressively.

These recordings are used as evidence. Losing the telemetry loses the part a claims handler would have
believed.

## The approach

**Read the container faithfully, and copy what is not understood.** Boxes are byte ranges unless a value is
actually needed. The vendor telemetry, the unknown top-level box and the decoder configuration are copied
verbatim, because what has to be preserved here is precisely what no library recognises
([ADR-0004](docs/adr/0004-boxes-are-opaque-byte-ranges.md)).

**One implementation of the format, six surfaces.** A Rust core links natively into the desktop and mobile
applications and compiles to WebAssembly for the website, so a file exported on a phone and a file exported
in a browser carry the same digest. Playback and composed encoding stay with each platform's own media
engine, where the hardware is ([ADR-0002](docs/adr/0002-one-rust-core-and-thin-shells.md)).

**Nothing leaves the device.** The website reads, plays, exports and checks entirely in the browser. One
small service sees digests and structured facts, never media
([ADR-0008](docs/adr/0008-the-website-runs-in-the-browser.md)).

**Report facts, never a verdict.** Every statement about integrity comes from a controlled catalogue, and no
surface composes its own. The product says what it observed and where the observation came from. It never
says a recording is authentic, because nothing can
([ADR-0005](docs/adr/0005-the-product-never-returns-a-verdict.md)).

## What it proves, and what it does not

| Statement | Provable | By what |
|---|---|---|
| The video samples of this extract are identical, byte for byte, to those of track N of this source | Yes | Equal track fingerprints, reproducible by a third party with ordinary tools |
| This file is the one adiungere saw, to the byte, when the manifest was written | Yes | Whole-file digest; originals are never rewritten |
| This file existed no later than a given instant | Yes, graded | A time stamp, with its legal weight stated rather than implied |
| The telemetry in the output is the telemetry of the source | Yes | Vendor bytes copied verbatim and digested separately |
| This composition derives from these sources | Yes | Source digests and fingerprints carried in the composition's manifest |
| The original recording was not altered before adiungere saw it | **No** | Nothing attests for the camera |
| What is shown actually happened | **No** | Beyond the reach of any tool |

The first four lines hold today: [`docs/integrity.md`](docs/integrity.md) defines the fingerprint, the
file digest and the vendor box digests, and shows how a stranger reproduces each of them with a digest
utility, a hundred-line reference script and the common media tool; a time-stamping authority's token can
be obtained over a manifest or carried inside a signature, and it is presented as the authority's word
with whatever standing that authority has. [`docs/verification-guide.md`](docs/verification-guide.md)
says the same for the person who has to decide, with the forensic bodies' own sentences on what an
authentication examination is and why this product performs none.

Four export classes carry that distinction permanently: an **extraction** copies samples byte for byte into a
rebuilt container; a **two-track archive** carries both cameras with no encoder; a **pixel-exact
rendition** earns its label only after every decoded frame has been compared; a **rendition** is re-encoded
and says so.

## Architecture

| Surface | Interface | Media engine | Export |
|---|---|---|---|
| iOS and macOS | One multiplatform declarative target | In-memory composition per view mode | Core writer, platform encoder for compositions |
| Android | Declarative toolkit on the current design language | Explicit track override, dual view behind a flag | Core writer, platform transformer for compositions |
| Windows | Thin shell hosting the shared player, core in process | Low-level decoders fed by the core | Core writer, platform encoder for compositions |
| Linux | Native toolkit shell | Two-branch pipeline with a zero-copy sink | Core writer, pipeline encoder for compositions |
| Web | Static installable application | Core in WebAssembly, low-level codecs with a tiered fallback | Core writer, browser encoder for compositions |

Lossless remuxing, fingerprints, the manifest and its verification always go through the core, including on
the platforms that could remux by themselves. Their writers drop the vendor telemetry box
([ADR-0006](docs/adr/0006-every-export-goes-through-the-core.md)).

[`docs/architecture.md`](docs/architecture.md) has the detail.

## Repository structure

Three tiers: what ships to a person, what the rest of the code consumes, and the supporting material.

| Path | Contents | Present |
|---|---|---|
| `apps/android` | The Android application | M7 |
| `apps/apple` | The iOS and macOS application | M8 |
| `apps/linux` | The Linux shell and its media pipeline | M5 |
| `apps/site` | adiungere.com | placeholder today, M4 |
| `apps/windows` | The Windows shell | M5 |
| `core` | The Rust workspace: the reader, the fingerprints, the manifest, the scanner, the command line, the synthetic corpus, the guarantee suite, the fuzzing project | **yes** |
| `web` | The shared player and export interface | M4 |
| `design` | The token source that generates every platform theme | M4 |
| `docs` | Roadmap, architecture, testing, integrity, evidence register, decision records | **yes** |
| `infra` | How an environment is described and deployed | M4 |
| `scripts` | The gate runner, the reference fingerprint script, the tool fetcher and the checks that do not belong to a crate | **yes** |
| `tools` | The pinned versions of the external tools the pipeline runs as oracles, and the oracle harnesses | **yes** |

## Guarantees

A guarantee is a property a test refuses to let anyone break, and **each one was watched failing against a
deliberate violation before it was trusted**. A guarantee is added in the milestone that adds the behaviour
it protects, never before: a test asserting a property of code that does not exist is a placeholder
reporting success.

### Enforced today

| Guarantee | What it refuses |
|---|---|
| G01 | A typographic dash in any tracked file, as a character, a look-alike or an HTML entity |
| G02 | An action referenced by a tag rather than a commit or a digest, or without its version in a comment |
| G03 | A directory holding source that no pipeline's own path list builds, with the directories discovered from git |
| G04 | A documentation link, in any form, that resolves to nothing, leaves the repository, or names a missing anchor |
| G05 | A path, an address, a credential or a key from a development or running environment reaching a tracked file |
| G06 | A file or a directory at the repository root that the root does not declare with a reason |
| G07 | A second toolchain pin, a pin that is not patch-exact, a minimum that disagrees with it, or a pipeline or script that overrides it |
| G08 | A licence declared inconsistently, a licence text that differs from its publication, or a policy admitting a licence outside the accepted set |
| G09 | Prose wider than 110 characters, which stops a document being reviewable as a diff |
| G10 | A directory that claims to be empty and is not, or is empty and does not say until when |
| G11 | Unsafe code, a panicking construct outside a test, or an attribute that switches either rule off |
| G12 | A reader whose ranges do not tile a recording, or re-emit it with a byte changed, unknown boxes included |
| G13 | A typed user-data box anywhere, or a vendor child that is not recoverable byte for byte after reading |
| G14 | A fingerprint the reference script does not reproduce, or a stream digest the pinned media tool does not |
| G15 | A parser without a fuzz target, a target the nightly does not run, or a panic in the mutation pass |
| G16 | A catalogue string that contains a forbidden word as a whole word, in any language on disk |
| G17 | A time without a clock from the closed set, or a time sentence that does not say "no later than" |
| G18 | A comparison that reports more, or less, than the one subject a flipped byte belongs to |
| G19 | A structural inspection that reads 256 kibibytes or more, or touches the media data |
| G20 | A probe the roadmap and the evidence register disagree about, or a register whose shape has drifted |
| G21 | An export that loses one byte of a recorder's box, writes the movie box after the media, or gives a track a fingerprint other than its source's |
| G22 | A writer whose output differs by one byte from the pinned digests, on any platform of the matrix |
| G23 | An export in which the pinned media tool counts other packets than the samples written, or decodes with a complaint |
| G24 | A reading crate that opens a file for writing, or a command that changes a recording's bytes or modification time |
| G25 | An encoder crate in the resolved graph, or an encoder symbol in the release command line |
| G26 | A sign the detector declares that no corpus recording raises, a sign without its phrase, or an unplaced file described as clean |
| G27 | A signed export the independent validator refuses, a rewrite after signing it accepts, or a re-signing it refuses |
| G28 | A recording whose bytes or time changed when credentials were written beside it, or a pair the validator refuses |
| G29 | A digest or an action that differs between the manifest beside a file, the signed facts inside it and the report |
| G30 | A signer name that is not the certificate's, a trust state that is not the validator's, or a fourth state |
| G52 | A count, a table or a documented gate step that no longer matches the repository |
| G54 | A tracked file the ignore file says never enters the repository, or a tracked file larger than source ever is |

The identifiers are stable, so the enforced set is not a contiguous range. Two arrived earlier than the plan
scheduled them: the evidence register was worth reading as data from the first day, and a count in a
document is wrong the moment nobody checks it. One was not in the plan: the platform cannot refuse a
recording or a key at the door for a repository owned by a person, so the suite does.

### Committed, with the milestone that will enforce each

Twenty-two further guarantees are written down against the milestone that will enforce them, from the
website of M4 to the production signer chain at M9.
[`docs/guarantees.md`](docs/guarantees.md) is the whole ledger.

## Evidence

Every load-bearing assumption in this project is a **probe**: a question, the method that answers it, and the
decision it settles. There are 41 of them, and the register says what is known about each in the only four
words it admits: `measured`, `reasoned`, `unavailable`, `not started`. The last two are never read as a pass.

```console
$ cargo run -p adiungere-cli -- probes list --milestone M1
$ cargo run -p adiungere-cli -- probes show P11
$ cargo run -p adiungere-cli -- probes check
```

[`docs/evidence.md`](docs/evidence.md) is the register, and the command reads it as data.

## Getting started

```console
$ git clone https://github.com/LennyObez/adiungere
$ cd adiungere/core
$ cargo build --release -p adiungere-cli
$ ./target/release/adiungere inspect clip.mp4
$ ./target/release/adiungere fingerprint clip.mp4 --manifest clip.manifest.json
$ ./target/release/adiungere verify clip.manifest.json clip.mp4
$ ./target/release/adiungere detect /media/card/
$ ./target/release/adiungere export clip.mp4 --camera rear --out rear.mp4
$ ./target/release/adiungere export front.mp4 rear.mp4 --out both.mp4
$ ./target/release/adiungere export clip.mp4 --camera rear --out rear.mp4 --sign --time-authority https://freetsa.org/tsr
$ ./target/release/adiungere sign clip.mp4
$ ./target/release/adiungere timestamp rear.mp4.manifest.json --time-authority https://freetsa.org/tsr
```

`inspect` reads the headers and never the media; `fingerprint` reads every byte and writes the manifest;
`verify` compares a manifest with a file, subject by subject; `detect` finds recordings in files and
directories and pairs the files of recorders that write one per camera; `report` renders a manifest with
the commands that reproduce each number. `export` writes one camera, or both, or the tracks named with
`--tracks`, copying every coded sample byte for byte, keeping the recorder's boxes, and placing the movie
box first; given two files it joins the front of the first and the video of the second into one two-track
file. It writes a manifest beside the output, reads the output back and refuses to keep it unless every
track carries its source's fingerprint. With `--sign` it signs last, the Content Credentials embedded in
the final file and the manifest inside them. `sign` writes credentials beside a recording, which it never
touches; `timestamp` asks a time-stamping authority for a token over a file; `verify` reads the credentials
a file carries and the token beside its manifest, and says what the validator found in three words that
never round up. Every command prints prose for a person or, with `--format json`, one document for a
pipeline, and none of them returns a verdict about a recording.

[`docs/getting-started.md`](docs/getting-started.md) has the tools, the gate sequence and where things are.

## Documentation

- [`docs/roadmap.md`](docs/roadmap.md), eleven milestones, each with a named demonstration
- [`docs/architecture.md`](docs/architecture.md), what owns what and why the boundary sits there
- [`docs/testing.md`](docs/testing.md), the method, the gates and how a guarantee is written
- [`docs/integrity.md`](docs/integrity.md), what is proved, how each number is defined, how to reproduce it
- [`docs/verification-guide.md`](docs/verification-guide.md), for the driver, the claims handler and the
  lawyer
- [`docs/evidence.md`](docs/evidence.md), the probe register
- [`docs/guarantees.md`](docs/guarantees.md), every guarantee and the milestone that enforces it
- [`docs/adr/`](docs/adr/README.md), the decisions, each with the check that makes it hold

## Licence

Apache-2.0, with a [`NOTICE`](NOTICE) file. There is no contributor agreement to sign: clause 5 of the
licence already settles it, and
[ADR-0003](docs/adr/0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md) says what that costs.
