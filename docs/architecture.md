# Architecture

One core, six surfaces, and each platform's own media engine. This document describes what owns what, and why
the boundary sits where it does.

> **This is the target architecture, not an inventory of what exists.** At the current milestone the
> repository holds its governance, its guarantee suite, and a command that reads the evidence register. Every
> component below arrives with the milestone named against it in [`roadmap.md`](roadmap.md). Sections in the
> present tense describe how the system is designed to work; where a mechanism is already in place, it says
> so.

## The problem this shape solves

A two-camera dashcam writes **one** file containing two video tracks and one audio track. Consumer galleries
and most players show the first video track and nothing else, because that is what their demultiplexers
select. Reaching the second camera today means a command line and an external tool, and that route throws
away the vendor telemetry box and the camera's identity along with it.

These recordings are used as evidence. So the product has to do three things without compromise: **find**
every dashcam clip in a library, **play and export** either camera or both, losslessly wherever that word can
honestly be used, and **prove** what it can prove about the bytes, while saying plainly what it cannot.

## Shape

The **core** is a Rust workspace. It is the only implementation of the container format in the product: box
reading, track inventory, telemetry decoding, per-track fingerprints, lossless remuxing, the integrity
manifest and its verification, and the rules that recognise and pair dashcam files. It links natively into
the desktop and mobile shells and compiles to WebAssembly for the website. A pipeline compares the native and
WebAssembly output byte for byte on the same fixture, so the proof is the same everywhere because the code is
the same everywhere.

**Playback and composed encoding stay with each platform**, because that is where the platform beats any
bundled library: hardware decoders, colour handling, power behaviour and accessibility all come free there
and cost dearly anywhere else. No media framework is bundled or required at run time. External media tools
are development-time oracles, never a dependency of the shipped product.

| Surface | Interface | Media engine | Library access | Composed export |
|---|---|---|---|---|
| iOS and macOS | One multiplatform declarative target | In-memory composition per view mode; direct sample reads for fingerprints | System photo library with a two-tier scan, picker, files | Platform writer with a video composition, audio passed through |
| Android | Declarative toolkit on the current design language | Explicit track override; dual view behind a flag with two measured backends | Photo picker by default, persisted document trees | Platform transformer with a capability ladder |
| Windows | Thin shell hosting the shared player, core linked in process | The shared low-level decoder player, fed by the core | Folders, drag and drop, file association | Platform encoder, muxed and finalised by the core |
| Linux | Native toolkit shell, core linked directly | Two-branch media pipeline with a zero-copy paintable sink, one clock | Folders through the portal, memory cards | Pipeline compositor and encoder, finalised by the core |
| Web | Static installable application | Core in WebAssembly for the container; low-level codecs for pictures and sound, with a tiered fallback | Files and folders chosen or dropped, nothing uploaded | Browser encoder, muxed by the core |

Lossless remuxing, fingerprints, the manifest and its verification always go through the core, including on
the platforms that can remux by themselves. Their writers would drop the vendor telemetry box.

## The core

| Crate | Responsibility | Milestone |
|---|---|---|
| `adiungere-isobmff` | Box tree as opaque ranges by default, sample iterator in decode order, one sans-I/O reader behind a synchronous and an asynchronous driver; remuxer writing a subset of tracks from one or two recordings with the recorder's boxes carried across as bytes; finalise step for a platform-encoded rendition | **present**; finalise M4 |
| `adiungere-fingerprint` | Track fingerprint, decoder configuration digest, secondary elementary-stream digest, file digest, structural fingerprint | **present** |
| `adiungere-scan` | Naming grammars by shape, bounded metadata probe, paired-file sources, the signs of a rewrite, a probe cache | **present** |
| `adiungere-manifest` | Manifest types, canonical serialisation, published schema, comparison as findings, the wording catalogue | **present** |
| `adiungere-fixtures` | The synthetic corpus, built from three committed streams with no encoder | **present** |
| `adiungere-provenance` | Content Credentials: ingredients, actions, the adiungere manifest as an assertion, embedded or sidecar placement, time stamping, reading back with the validator's state, and a delegated signer for the service | **present** |
| `adiungere-telemetry` | Read-only, best-effort vendor telemetry parser that degrades to bytes preserved | M6 |
| `adiungere-core` | The public API facade, written when the first surface other than the command line consumes it, so that it is shaped by a consumer rather than guessed | M4 |
| `adiungere-ffi` | Foreign function surface for Swift and Kotlin, with generated bindings committed | M7 |
| `adiungere-wasm` | Browser surface with a segmented reader and writer | M4 |
| `adiungere-cli` | The executable specification: inspect, fingerprint, detect, verify, report, export, probes | **present** |
| `adiungere-relay` | The minimal signing and time-stamping service | M4 |
| `adiungere-guarantees` | The repository guarantee suite | **present** |

Two rules shape every one of them. **Boxes are byte ranges unless a value is needed**, because a typed
container silently drops the children it does not recognise, and the children it does not recognise are
exactly the vendor telemetry this product exists to preserve. **The core does not panic by design**: the
constructs that abort on purpose are denied outside tests, and arithmetic is checked so that an overflow is
a defect that surfaces rather than a wrong offset that ships. It runs inside six host applications that must
report a bad file rather than die on one.

## What the product proves

| Statement | Provable | By what |
|---|---|---|
| The video samples of this extract are identical, byte for byte, to those of track N of this source | Yes | Equal track fingerprints, reproducible by a third party with ordinary tools |
| This file is the one the product saw, to the byte, when the manifest was written | Yes | Whole-file digest in the manifest; originals are never rewritten |
| This file existed no later than a given instant | Yes, graded | A time stamp, with its legal weight stated rather than implied |
| The telemetry in the output is the telemetry of the source | Yes | Vendor bytes copied verbatim and digested separately |
| This composition derives from these sources | Yes | Source digests and fingerprints carried in the composition's manifest |
| The original recording was not altered before the product saw it | **No** | Nothing attests for the camera. The report says what the structure shows, never "authentic" |
| What is shown actually happened | **No** | Beyond the reach of any tool |

Four export classes carry that distinction permanently: an **extraction** copies samples byte for byte into a
rebuilt container; a **two-track archive** carries both cameras with no encoder; a **pixel-exact rendition**
earns its label only after every decoded frame has been compared; a **rendition** is re-encoded and says so.
The integrity label shown to a person derives from that classification and from nothing else.

The wording is a controlled vocabulary held in a catalogue and enforced by a test, so no surface can promote
a fact into a verdict. The specification of the fingerprint and the manifest, with a reference implementation
short enough to read in one sitting, is published in [`docs/integrity.md`](integrity.md).

## Trust boundaries

Each control arrives with the milestone that opens the boundary it guards. None exists yet, because no code
has crossed one.

| Boundary | Control | Milestone |
|---|---|---|
| A recording on disk to the reader | Read-only handles; the source is never opened for writing on any surface | M2 |
| Media to the network | Nothing leaves the device. Only digests and structured claim facts reach the signing service | M4 |
| Signing service to a claim | Claims are rebuilt from structured facts, never signed as opaque bytes; bodies are capped | M4 |
| A person to a share action | Faces and plates masked, or an explicit acknowledgement, before any share affordance | M6 |
| Vendor telemetry to interpretation | Preservation never depends on interpretation; an unknown layout degrades to bytes preserved | M6 |

## The website

A static application. Recordings are read, played and exported entirely in the browser, which is the whole
argument for a tool whose output may end up in a claim file or a court bundle. One small service sits behind
the site and sees **only digests and structured facts**, never media: it relays a time stamp request that a
browser cannot make itself, and it holds the single signing key, because a key shipped inside a downloadable
binary is a key anyone can extract.

The verification surface is called **Check**. It is client-side: a reader drops a file and its manifest, the
core recomputes, and the two are compared without an account and without an upload.

## What exists today

The Rust workspace with the reader, the fingerprints, the manifest, the scanner, the remuxer and the
provenance; a command line that inspects, fingerprints, detects, verifies, reports, exports, signs and
time-stamps; the synthetic corpus; the guarantee suite; the evidence register and the command that reads
it; the governance of the repository; and a static page that says honestly that the product is not built
yet. Nothing plays a recording, every signature is made with a credential on no trust list, the signing
service does not exist yet, and no surface other than the command line exists. Everything else in this
document is a commitment with a milestone against it.
