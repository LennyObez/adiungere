# adiungere

A dashcam with two cameras writes **one** file holding both of them. Galleries and players show the first
camera and nothing else. adiungere finds those recordings, plays either camera or both together, exports
each one without re-encoding it, and keeps the vendor telemetry that every other route throws away.

The name is Latin: to join, to attach to. Two views of one moment, joined rather than separated.

**Status: early construction.** The foundation is in place; the product is not yet usable. What follows
describes the product being built, and [Guarantees](#guarantees) separates the properties a test enforces
today from those committed to, each against the milestone that will enforce it.
[`docs/roadmap.md`](docs/roadmap.md) holds the plan.

---

## The problem

A two-camera dashcam does not write two files. It writes one file with two video tracks and one audio track,
and every consumer gallery shows the first track, because that is what its demultiplexer selects. The rear
camera is in the file. Nothing offers to show it.

Reaching it today means a command line and an external tool. That route works, and it silently throws away
the vendor telemetry box: on a measured reference recording, roughly thirty kilobytes holding the camera
identity, one satellite fix per second and about twenty motion records per second. It also rewrites the
container in a way that stops the file playing progressively.

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

Four export classes carry that distinction permanently: an **extraction** copies samples byte for byte into a
rebuilt container; a **two-track archive** carries both cameras with no encoder; a **verified lossless
rendition** earns its label only after decoded frames are compared; a **rendition** is re-encoded and says
so.

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
| `core` | The Rust workspace: the format, the fingerprints, the manifest, the command line | **yes** |
| `web` | The shared player and export interface | M4 |
| `design` | The token source that generates every platform theme | M4 |
| `docs` | Roadmap, architecture, testing, evidence register, decision records | **yes** |
| `infra` | How an environment is described and deployed | M4 |
| `scripts` | The gate runner and the checks that do not belong to a crate | **yes** |

## Guarantees

A guarantee is a property a test refuses to let anyone break, and **each one was watched failing against a
deliberate violation before it was trusted**. A guarantee is added in the milestone that adds the behaviour
it protects, never before: a test asserting a property of code that does not exist is a placeholder
reporting success.

### Enforced today

| Guarantee | What it refuses |
|---|---|
| G01 | A typographic dash in any tracked file |
| G02 | A workflow action referenced by a tag rather than a commit, or without its version in a comment |
| G03 | A directory holding source that no pipeline builds, with the directories discovered from git |
| G04 | A relative documentation link that resolves to nothing, or a decision record missing from the index |
| G05 | A path, an address or a private key from a development environment reaching a tracked file |
| G06 | A file at the repository root that the root does not declare |
| G07 | A second toolchain pin, a pin that is not patch-exact, or a pipeline that restates the version |
| G08 | A licence declared inconsistently, an altered licence text, or a reciprocal licence in the policy |
| G09 | Prose wider than 110 columns, which stops a document being reviewable as a diff |
| G10 | A directory that claims to be empty and is not, or is empty and does not say until when |
| G11 | Unsafe code, or a panicking construct outside a test |
| G20 | A probe the roadmap and the evidence register disagree about |
| G52 | A count or a table in the documentation that no longer matches the repository |

The identifiers are stable, so the enforced set is not a contiguous range. The last two arrived earlier than
the plan scheduled them: the evidence register was worth reading as data from the first day, and the table
reconciliation caught a wrong number in this README on the day it was written.

### Committed, with the milestone that will enforce each

Thirty-nine further guarantees are written down against the milestone that will enforce them, from the
byte-for-byte round trip at M1 to the masking gate at M6.
[`docs/guarantees.md`](docs/guarantees.md) is the whole ledger.

## Evidence

Every load-bearing assumption in this project is a **probe**: a question, the method that answers it, and the
decision it settles. There are 36 of them, and none is answered yet, which the register says in the only four
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
$ cargo test --workspace
```

[`docs/getting-started.md`](docs/getting-started.md) has the tools, the gate sequence and where things are.

## Documentation

- [`docs/roadmap.md`](docs/roadmap.md), eleven milestones, each with a named demonstration
- [`docs/architecture.md`](docs/architecture.md), what owns what and why the boundary sits there
- [`docs/testing.md`](docs/testing.md), the method, the gates and how a guarantee is written
- [`docs/evidence.md`](docs/evidence.md), the probe register
- [`docs/guarantees.md`](docs/guarantees.md), every guarantee and the milestone that enforces it
- [`docs/adr/`](docs/adr/README.md), the decisions, each with the check that makes it hold

## Licence

Apache-2.0, with a [`NOTICE`](NOTICE) file. There is no contributor agreement to sign: clause 5 of the
licence already settles it, and
[ADR-0003](docs/adr/0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md) says what that costs.
