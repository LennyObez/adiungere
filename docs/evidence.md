# Evidence register

Every load-bearing assumption in this project is a **probe**: a question, the method that answers it, and the
decision it settles. A probe is recorded here with a verdict, and a verdict is one of four words.

| Verdict | Meaning |
|---|---|
| `measured` | The method was run and produced a result. The result is written below, with the command. |
| `reasoned` | No measurement was possible, and the conclusion rests on a primary source quoted below. |
| `unavailable` | The method exists and could not be run. It says which hardware, account or access is missing. |
| `not started` | Nobody has run it. It is not evidence of anything. |

`unavailable` and `not started` are **never** read as a pass. A decision that depends on an unanswered probe
carries its fallback in the open, or it waits.

## How this file is read

This register is data as well as prose. `adiungere probes` parses it, and refuses a file whose structure has
drifted rather than quietly reading less than it should.

Every entry is a level-two heading of the form `## PNN Title`, followed by exactly these bullets, in this
order, each on one line:

```markdown
## P00 A title in sentence case

- **Verdict:** not started
- **Milestone:** M1
- **Question:** what is actually unknown, phrased so that an answer can be wrong.
- **Method:** what would be run, precisely enough that someone else could run it.
- **Decides:** the decision, guarantee or milestone item that changes with the answer.
```

A `measured` or `reasoned` entry adds a sixth bullet, `- **Result:**`, followed by as much prose as the
finding needs. Anything else in the file is prose and is ignored.

Run `cargo run -p adiungere-cli -- probes check` to reconcile this register with the roadmap.

---

## P01 Platform readers produce the same track fingerprint as the core

- **Verdict:** not started
- **Milestone:** M1
- **Question:** do the platform sample readers hand back the stored sample bytes unchanged, so that a
  fingerprint computed on a phone equals one computed by the command line?
- **Method:** digest the reference clip with the core, then with a harness over each platform reader, and
  compare. The stored length prefix is four bytes on the reference clip, which is the precondition.
- **Decides:** the shape of the fingerprint recipe, and whether platform readers can serve as oracles.

## P02 Vendor box placement, internal offsets and the four-byte vendor code

- **Verdict:** not started
- **Milestone:** M1
- **Question:** is the telemetry box a direct child of the user-data box, does its payload contain absolute
  file offsets, and what does the four-byte top-level vendor code mean across manufacturers?
- **Method:** dump the box tree; diff the telemetry box between two exports whose media data sits at
  different offsets; compare the vendor code against the values other cameras are documented to emit.
- **Decides:** whether vendor bytes can be copied verbatim, and whether the top-level box is reinserted or
  recorded as deliberately absent.

## P03 An independent parser reproduces the file byte for byte

- **Verdict:** not started
- **Milestone:** M1
- **Question:** does an independent box library round-trip the reference clip without editing it, and does an
  independent dumper agree with our reader about the box tree?
- **Method:** round-trip with no edit command, digest before and after, diff the two box trees.
- **Decides:** which independent oracle the pipeline runs against our own reader.

## P04 A sidecar signature leaves the asset untouched

- **Verdict:** not started
- **Milestone:** M3
- **Question:** does producing an external provenance manifest for an original leave that original identical
  to the byte, and does an external validator accept the pair?
- **Method:** digest before and after; validate with the external tool using its external-manifest option.
- **Decides:** whether originals can carry provenance at all, since the product never rewrites an original.

## P05 Which identifier an embedded manifest is stored under

- **Verdict:** not started
- **Milestone:** M3
- **Question:** which box identifier does the current provenance specification reserve for a manifest inside
  an ISO base media file, and which one does the library actually emit?
- **Method:** read the specification annex; inspect a signed output.
- **Decides:** the emission settings, and what the reader looks for.

## P06 What Safari does with two enabled video tracks

- **Verdict:** not started
- **Milestone:** M4
- **Question:** which track does a media element render when two video tracks are both marked enabled, does
  the audio still play, does a large object URL load, and does the track list expose both?
- **Method:** a test page per engine, on a real device for the mobile case, with a frame comparison.
- **Decides:** whether the hybrid player tier is allowed on that engine, or a full decoder path is forced.

## P07 Desktop Firefox decodes and encodes the profile in use

- **Verdict:** not started
- **Milestone:** M4
- **Question:** does desktop Firefox decode and encode this exact profile through the low-level codec API,
  given length-prefixed packets and a decoder configuration record?
- **Method:** a real decode and a real encode, not a support query, on Windows, macOS and Linux.
- **Decides:** how deep the fallback tier must go, and whether composed export runs on that engine.

## P08 Two simultaneous decodes and a composite, on modest hardware

- **Verdict:** not started
- **Milestone:** M4
- **Question:** can a modest laptop, a mid-range phone and an older phone sustain two 1080p30 decodes plus a
  canvas composite for a minute without dropping frames, and what does the hybrid path cost?
- **Method:** a throwaway page reporting dropped frames, processor use, memory and thermal state over sixty
  seconds. Include a Windows edition without the media feature pack.
- **Decides:** whether the dual view is a default or a per-device capability, and the Windows fallback.

## P09 The Linux webview fallback can decode at all

- **Verdict:** not started
- **Milestone:** M5
- **Question:** inside a sandboxed package with the codec extension present, does the Linux webview decode
  this profile through the low-level codec API, can it hold two decodes, and which audio route works?
- **Method:** a minimal shell built as a sandboxed package on clean Fedora and Ubuntu machines.
- **Decides:** whether the Linux fallback shell is viable if the native pipeline probe fails.

## P10 The WebAssembly writer streams within a memory budget

- **Verdict:** not started
- **Milestone:** M4
- **Question:** does the remuxer compiled to WebAssembly stream a 140 MB file from sliced reads with bounded
  memory, and produce output identical to the native build?
- **Method:** build the writer, digest the output, record peak memory on three engines.
- **Decides:** whether the website exports through our own engine or a third-party fragmenter.

## P11 The provenance library supports a split signing flow

- **Verdict:** not started
- **Milestone:** M3
- **Question:** can the signing service rebuild a claim from structured facts and sign it without ever seeing
  the media, and what exactly must the client send?
- **Method:** read the builder and signer surfaces; prototype a service and a client against the reference
  clip.
- **Decides:** single key custody at the service, or timestamped manifests with no Content Credentials.
  This is the heaviest unknown in the plan: it has no degraded mode that keeps the advertised product.

## P12 Provenance validation inside the browser

- **Verdict:** not started
- **Milestone:** M4
- **Question:** does the provenance library compiled to WebAssembly validate an embedded manifest read
  through a segmented reader, and can it embed one in the browser?
- **Method:** build both paths and run them against a signed export.
- **Decides:** whether the check page validates locally or prints the command a reader can run themselves.

## P13 What the Apple photo library does to an import, a cancellation and vendor metadata

- **Verdict:** not started
- **Milestone:** M8
- **Question:** does adding a file to the photo library re-encode it, does cancelling a request actually stop
  a cloud download, and does the framework expose the vendor user-data box?
- **Method:** a harness on a runner and on a device.
- **Decides:** whether exports may be written back to the library, how cheap a metadata probe is, and whether
  our own parser is the only reader of vendor data.

## P14 Whether a cloud-only video yields its original bytes

- **Verdict:** not started
- **Milestone:** M7
- **Question:** does the Android picker's cloud provider return the original bytes of a video that exists
  only in the cloud, or a transcoded derivative?
- **Method:** upload the reference clip, remove the local copy, pick it, compare digests.
- **Decides:** every claim the product makes about cloud libraries on Android. The default claim is none.

## P15 Whether the clock disagreement is constant

- **Verdict:** not started
- **Milestone:** M1
- **Question:** across a second and a third recording, is the offset between the container clock and the
  satellite clock constant, is the telemetry layout stable, and are there absolute offsets inside it?
- **Method:** obtain further recordings, including the other event types, inspect and parse.
- **Decides:** how the clock disagreement is worded, whether verbatim copying is safe, and how far the
  telemetry parser may go. Until it is answered, the wording says only that the clocks disagree.

## P16 Default track selection and per-track export on Android

- **Verdict:** not started
- **Milestone:** M7
- **Question:** without an override, does the player pick the first video track or the highest-bitrate one,
  and can the transformer select one specific track of a multi-track file?
- **Method:** a test on a device.
- **Decides:** the dual view backend on Android only.

## P17 Cost, eligibility and lead time of the signing arrangements

- **Verdict:** not started
- **Milestone:** M5
- **Question:** is this project eligible for free code signing for open source, what does a qualified time
  stamp cost per unit and in bulk, and what does a provenance certificate cost, take and require?
- **Method:** apply, request quotes, record dated facts.
- **Decides:** the Windows signing route and the shape of the evidence tier.

## P18 Sustained cost of the dual view on the oldest supported devices

- **Verdict:** not started
- **Milestone:** M7
- **Question:** what does a minute of two 1080p30 decodes plus a wide composite cost on the oldest supported
  phone of each platform?
- **Method:** a sixty-second harness reporting frames, processor, memory and thermal state.
- **Decides:** the device guard on the dual view and the preview render size.

## P19 What the host can serve and how it deploys

- **Verdict:** not started
- **Milestone:** M0
- **Question:** does the host serve WebAssembly with the correct media type, set long caches for immutable
  assets, allow the security headers this project needs, and deploy from the default branch without granting
  anything beyond the site directory?
- **Method:** deploy the placeholder with a test asset and inspect the response headers from outside.
- **Decides:** the shape of the deployment and, later, whether the signing relay runs on this host.

## P20 Whether a practitioner reproduces the numbers and reads the wording as intended

- **Verdict:** not started
- **Milestone:** M9
- **Question:** can a lawyer and a claims handler, given the guide and ordinary tools, reproduce the numbers
  in a manifest, and do they read the wording the way it is meant?
- **Method:** two informal read-throughs, with every misunderstanding recorded verbatim.
- **Decides:** the wording catalogue and the verification guide.

## P21 Whether design tokens arrive as structured values

- **Verdict:** not started
- **Milestone:** M4
- **Question:** does the design tooling hand off structured tokens that a generator can read, or only markup
  from which a human must transcribe them?
- **Method:** publish a test system, take one artefact through each handoff route.
- **Decides:** whether the token generators consume an export or a table written by hand, and whether each
  platform brief has to restate the tokens.

## P22 Feeding demultiplexed packets to the Windows shell

- **Verdict:** not started
- **Milestone:** M5
- **Question:** can the shell stream demultiplexed packets to its webview at 20 Mbit/s with seeks under a
  tenth of a second, or must a file handle be passed to a demultiplexer inside the page?
- **Method:** measure both routes on Windows.
- **Decides:** whether one demultiplexer serves every surface, or the desktop keeps a second one.

## P23 A two-branch zero-copy pipeline under the Linux toolkit

- **Verdict:** not started
- **Milestone:** M5
- **Question:** does the toolkit's paintable sink give a two-branch pipeline without copying frames, on both
  display protocols, across the three graphics vendors, inside the sandbox?
- **Method:** a spike built as a sandboxed package, reporting dropped frames and processor use.
- **Decides:** the native Linux shell, or the fallback shell.

## P24 Whether the sandboxed codec extension decodes this profile

- **Verdict:** not started
- **Milestone:** M5
- **Question:** does the runtime codec extension decode High profile on stock Fedora and Ubuntu, and what
  does the application do when a user masks it?
- **Method:** install the spike on fresh virtual machines, then mask the extension.
- **Decides:** whether the sandboxed package is the main Linux channel, and the wording of the missing-codec
  state.

## P25 Whether a time-stamping authority answers a browser

- **Verdict:** not started
- **Milestone:** M1
- **Question:** do candidate time-stamping authorities answer cross-origin requests from a browser?
- **Method:** issue a request from a page on the production origin.
- **Decides:** whether the relay needs a time-stamping role at all.

## P26 What the forensic and legal sources actually say

- **Verdict:** not started
- **Milestone:** M3
- **Question:** is there a named judgment behind the claim that a court may not set aside dashcam video on
  suspicion alone, and what do the two forensic bodies say word for word about authentication conclusions?
- **Method:** read the primary sources and quote them here.
- **Decides:** every legal sentence on the website and in the report. Until it is answered, none is written.

## P27 Written licence opinion for the Linux media stack

- **Verdict:** not started
- **Milestone:** M1
- **Question:** what are the exact terms for dynamically linking the media framework from a sandboxed
  runtime, for the language bindings, and for the native window materials on Windows?
- **Method:** read the texts, write an opinion, record it here.
- **Decides:** the dependency policy exception and the desktop decision record.

## P28 The largest file a browser will hand back through a link

- **Verdict:** not started
- **Milestone:** M4
- **Question:** how large an in-memory output can Safari and Firefox deliver through a download link before
  the tab dies?
- **Method:** produce outputs of 500 MB, 1, 2 and 4 GB in the real application.
- **Decides:** the warning thresholds and when a large export is steered towards an installed application.

## P29 Whether an iPhone can hand over a folder

- **Verdict:** not started
- **Milestone:** M4
- **Question:** does the directory input on iOS Safari reach the files application and cloud storage?
- **Method:** a manual test on a device.
- **Decides:** whether folder ingest may be claimed on that platform.

## P30 Whether the broad media permission is approved

- **Verdict:** not started
- **Milestone:** M7
- **Question:** does the store approve a broad video-read permission declaration for an application of this
  kind?
- **Method:** submit the declaration with an internal test build.
- **Decides:** library-wide scanning on Android, or a documented limitation.

## P31 A native Windows media path, if that upgrade is taken

- **Verdict:** not started
- **Milestone:** M5
- **Question:** does the native Windows media stack expose both streams, decode them in parallel on the
  graphics device, and present two 1080p30 surfaces without dropping frames?
- **Method:** a spike, measured over sixty seconds, including an edition without the media feature pack.
- **Decides:** whether a native Windows shell replaces the webview shell.

## P32 Which accessibility standard version binds the installed applications

- **Verdict:** not started
- **Milestone:** M4
- **Question:** which harmonised version of the European accessibility standard applies to software today,
  and does a webview-hosted desktop application fall under its software clause or its web clause?
- **Method:** read the current harmonised text, write the opinion here.
- **Decides:** the accessibility floor stated in the design briefs and enforced by the contrast guarantee.

## P33 An on-device detector for faces and plates

- **Verdict:** not started
- **Milestone:** M6
- **Question:** which face and plate detector carries a permissive licence, needs no proprietary services,
  and runs on device across all six surfaces?
- **Method:** inventory the candidates, read the licences, test on the reference clip.
- **Decides:** automatic masking, or a manual region tool shipped and described as such.

## P34 Exactly what the elementary stream recipe produces

- **Verdict:** not started
- **Milestone:** M1
- **Question:** what does the documented third-party command actually write, in particular where it inserts
  parameter sets, and is that stable between versions of the tool?
- **Method:** compare the output of two versions of the tool on the reference clip and specify the recipe.
- **Decides:** the definition of the secondary digest, and the pinned tool version.

## P35 The real minimum compiler versions of the dependency set

- **Verdict:** not started
- **Milestone:** M1
- **Question:** what minimum compiler version does each dependency in the eventual set require, and is a
  single pin compatible with all of them?
- **Method:** read the manifests, compile against the candidate pin.
- **Decides:** the pinned toolchain and the lint configuration.

## P36 Whether the core builds everywhere it is promised

- **Verdict:** not started
- **Milestone:** M1
- **Question:** does the core, including the provenance library and its cryptography and transport stack,
  compile and pass its tests on Windows, both macOS architectures, a static Linux target and WebAssembly?
- **Method:** a build matrix, from the milestone that first has code worth building.
- **Decides:** the supported platform matrix and the feasibility of the browser export engine.
