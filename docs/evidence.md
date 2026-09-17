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

Every entry is a level-two heading of the form `## PNN Title`, with two or more digits and never zero,
followed by exactly these bullets, in this order. A bullet may continue on the lines directly below it, each
indented by two spaces; a blank line ends it.

```markdown
## P99 A title in sentence case

- **Verdict:** not started
- **Milestone:** M1
- **Question:** what is actually unknown, phrased so that an answer can be wrong, and wrapped onto a
  second line when it is long.
- **Method:** what would be run, precisely enough that someone else could run it.
- **Decides:** the decision, guarantee or milestone item that changes with the answer.
```

A `measured` or `reasoned` entry adds a sixth bullet, `- **Result:**`, followed by as much prose as the
finding needs, including fenced blocks for the commands that produced it, up to the next heading. Prose
between entries is ignored; a heading that starts like an entry and is not one, a field bullet outside any
entry, and a fenced block that never closes are each refused, because each is how an entry vanishes from the
count without anyone noticing.

Run `cargo run -p adiungere-cli -- probes check` to reconcile this register with the roadmap.

---

## P01 Platform readers produce the same track fingerprint as the core

- **Verdict:** measured
- **Milestone:** M1
- **Question:** do the platform sample readers hand back the stored sample bytes unchanged, so that a
  fingerprint computed on a phone equals one computed by the command line?
- **Method:** digest the reference clip with the core, then with a harness over each platform reader, and
  compare. The stored length prefix is four bytes on the reference clip, which is the precondition.
- **Decides:** the shape of the fingerprint recipe, and whether platform readers can serve as oracles.
- **Result:** measured on 2026-09-15 for the browser-side reader, on the reference recording and on every
  recording of the synthetic corpus; the Apple reader is measured by the oracles pipeline on the corpus,
  through the harness in `tools/oracles/platform-reader.swift`, and this entry is extended with its answer
  the first time that pipeline runs.

  The browser-side demuxing library, at the version pinned in `tools/oracles/package.json`, hands back
  every packet exactly as stored: the SHA-256 of its packets concatenated in decode order equals the track
  fingerprint's payload digest on all three tracks of the reference recording and on all twenty-six tracks
  of the corpus, under every prefix width the corpus exercises. Its decoder description for video is the
  decoder configuration record itself, so its digest equals the fingerprint's configuration digest. For
  audio it exposes the two-byte specific configuration rather than the whole elementary stream descriptor
  the fingerprint digests, so that digest is not compared for audio; the payload digest is.

  Consequence: the fingerprint recipe stands as defined, and the browser reader can serve as an oracle
  and, at M4, as the demuxer behind the website's player without any concern that what it decodes differs
  from what the core fingerprints.

  ```console
  $ scripts/oracles.sh browser /tmp/corpus
  oracles: the browser reader agrees with the product on all 26 tracks
  ```

## P02 Vendor box placement, internal offsets and the four-byte vendor code

- **Verdict:** measured
- **Milestone:** M1
- **Question:** is the telemetry box a direct child of the user-data box, does its payload contain absolute
  file offsets, and what does the four-byte top-level vendor code mean across manufacturers?
- **Method:** dump the box tree; diff the telemetry box between two exports whose media data sits at
  different offsets; compare the vendor code against the values other cameras are documented to emit.
- **Decides:** whether vendor bytes can be copied verbatim, and whether the top-level box is reinserted or
  recorded as deliberately absent.
- **Result:** measured on 2026-09-15 on the reference recording, with the product's reader and with the
  reference script.

  The telemetry box is a direct child of the user-data box under the movie box, at offset 53 439, of
  30 720 bytes. The user-data box holds one more child: a standard requirement atom of 36 bytes naming the
  player generation the file expects, which every general-purpose writer also drops. Nothing under the
  user-data box is a container.

  The offset question was answered without a second export, which no writer exists to make yet: every
  32-bit value in the telemetry payload, read at every byte position in both byte orders, was compared
  with every chunk offset of every track and every top-level box offset, 6 185 distinct offsets past the
  first four kibibytes. None matched, and the four small box offsets, movie, telemetry, model code and
  media data, do not appear either. The payload carries no absolute file offset, so a verbatim copy is
  safe at any position in an output file. M2 copies it verbatim and sets no `offsets_unadjusted` flag,
  because there is nothing to adjust.

  The top-level vendor box is twelve bytes whose four-byte payload is the ASCII digits `6350`: a model
  code, not a container. The exports of M2 copy it verbatim at the same position relative to the movie box,
  because a box a recorder writes is a box the recorder's own tools may look for.

  ```console
  $ adiungere inspect <recording> --format json | jq '.vendor'
  ```

## P03 An independent parser reproduces the file byte for byte

- **Verdict:** measured
- **Milestone:** M1
- **Question:** does an independent box library round-trip the reference clip without editing it, and does an
  independent dumper agree with our reader about the box tree?
- **Method:** round-trip with no edit command, digest before and after, diff the two box trees.
- **Decides:** which independent oracle the pipeline runs against our own reader.
- **Result:** measured on 2026-09-15 with the independent box editor at version 0.13.0 of its crate, on the
  reference recording and on the reference-like synthetic recording.

  An edit-less round trip reproduces both files byte for byte: the editor reports zero chunk offsets
  adjusted and the digest after equals the digest before, `02e43366…4278` for the reference recording. Its
  dumper lists the same boxes at the same offsets and sizes as the product's reader, top level and movie
  box alike, and its sample lister places all 1800 samples of the second video track at the same offsets
  with the same sizes as the reference script's placement, which is the placement the fingerprint digests.

  Consequence: the editor is the independent oracle for M2, where an extraction's sample tables are
  checked by a parser this project did not write. It is a developer tool, pinned by version, never a
  dependency of the product.

  ```console
  $ mp4edit <recording> /tmp/round-trip.mp4
  $ sha256sum <recording> /tmp/round-trip.mp4
  $ mp4samples --json --track-id 2 <recording>
  ```

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

- **Verdict:** measured
- **Milestone:** M1
- **Question:** across a second and a third recording, is the offset between the container clock and the
  satellite clock constant, is the telemetry layout stable, and are there absolute offsets inside it?
- **Method:** obtain further recordings, including the other event types, inspect and parse.
- **Decides:** how the clock disagreement is worded, whether verbatim copying is safe, and how far the
  telemetry parser may go. Until it is answered, the wording says only that the clocks disagree.
- **Result:** measured on 2026-09-15 on three original recordings from the same camera and firmware string,
  two consecutive event recordings from one winter session and the reference recording from a summer
  session. The offset is **not** constant, and the reason is more useful than a constant would have been:
  the container clock does not describe the recording at all. The recordings are private, so the table
  gives each clock relative to the time in the file name rather than as a date.

  | Recording | Satellite clock, UTC, relative to the file name | Container clock, field defined as UTC, relative to the file name |
  |---|---|---|
  | Winter, first of a pair | one hour earlier, plus one second | six minutes and one second earlier |
  | Winter, one minute later | one hour earlier, plus one second | seven minutes and one second earlier, the same value as above |
  | Summer, the reference | two hours earlier, plus two seconds | twenty-five minutes earlier |

  The file name and the satellite clock agree to within two seconds on every recording, once the local
  offset is applied: one hour in winter, two in summer. The two consecutive winter recordings carry the
  **same** container time, six and seven minutes before either starts, and the summer one carries a
  container time twenty-five minutes before it starts. So the container clock is a session clock, written
  once when the camera powers on, in local time, into a field the format defines as UTC, and reused for
  every file of the session. It never identifies a recording. The motion records carry the local time of
  the file name, to the second, in their first six bytes after the record tag.

  The telemetry layout is identical across the three files at this firmware string: the box sits at the
  same offset in each, opens with the same camera identity, holds one satellite sentence per second, and its
  binary records are sixteen bytes each. Whether any field inside it is an absolute file offset is the part
  of the question the box reader answers, and it is carried by P02.

  Consequence for the wording: no surface shows the container clock as the recording's time. The recording
  time shown is the satellite clock, with its source named, and the file name is shown as what the camera
  called the file. Two recording types remain unmeasured, the manual and the ordinary ones, and the
  fixture corpus of M1 states that.

  Commands, run against the recordings named above with the container probe tool at the version pinned in
  M1 and a short script over the raw bytes of the telemetry box:

  ```console
  $ ffprobe -v error -show_entries format_tags=creation_time -of default=nw=1:nk=1 <recording>
  $ python3 -c 'import re,sys; d=open(sys.argv[1],"rb").read(400000); i=d.find(b"mamt"); \
      print(re.findall(rb"\$GNRMC,(\d{6})\.?\d*,A,.*?,(\d{6}),", d[i:i+40000])[:1])' <recording>
  ```

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

- **Verdict:** measured
- **Milestone:** M0
- **Question:** does the host serve WebAssembly with the correct media type, set long caches for immutable
  assets, allow the security headers this project needs, and deploy from the default branch without granting
  anything beyond the site directory?
- **Method:** deploy the placeholder with a test asset and inspect the response headers from outside.
- **Decides:** the shape of the deployment and, later, whether the signing relay runs on this host.
- **Result:** measured on 2026-09-10 and 2026-09-15 from outside, after the host was pointed at the
  repository.

  **The deployment works, and it grants nothing beyond the site directory.** The page served at the root is
  the repository's own, byte for byte, and its modification time follows the push. The host pulls the
  default branch into a directory outside the served tree and the document root is the published
  directory inside it, so nothing of the repository is reachable: the README, the documentation, the
  workspace and the version history all answer 404. One answer needs reading correctly: a path ending in a
  well-known dependency manifest name answers 403 whether or not it exists, which is a rule of the host
  refusing such names anywhere, not evidence that the file is there.

  **Transport is right.** HTTP/2, a valid certificate, and strict transport security already set for two
  years including subdomains.

  **The response headers this project wants are not there yet.** No content security policy, no
  `X-Content-Type-Options`, no `Referrer-Policy`. Setting them is a host action, written down outside the
  repository, and the page's own claim of loading nothing from another host rests on a check over its
  source until they are.

  **No cache lifetime is set** on a static file. Without consequence today, since the page is self-contained.

  **The security contact the domain serves is not the repository's.** A tool on the host writes its own
  file at the well-known path, with a different expiry and a canonical location naming another domain, which
  makes it inapplicable to this one. Until the host serves the tracked file, `SECURITY.md` points at a
  contact the repository does not control. This is the one defect the deployment introduced, and it is open.

  **Two observations could not be taken** because they need an artefact that does not exist yet: the media
  type the host returns for a WebAssembly file, and the cache lifetime of an asset with a hashed name. Both
  are carried by P38 and are measured when the first such asset is published.

  ```console
  $ curl -sS -D - -o /dev/null https://adiungere.com/
  $ curl -sS -o /dev/null -w '%{http_code}\n' https://adiungere.com/README.md
  $ curl -sS -o /dev/null -w '%{http_code}\n' https://adiungere.com/core/
  $ curl -sS https://adiungere.com/.well-known/security.txt
  ```

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

- **Verdict:** measured
- **Milestone:** M1
- **Question:** do candidate time-stamping authorities answer cross-origin requests from a browser?
- **Method:** issue a request from a page on the production origin.
- **Decides:** whether the relay needs a time-stamping role at all.
- **Result:** measured on 2026-09-15 with a preflight request carrying the production origin, against
  three public authorities: a free one, and two run by certificate authorities. None answered with an
  `Access-Control-Allow-Origin` header. One refused the preflight outright, one answered that the method is
  not implemented, and one answered the preflight with the methods it accepts and no origin header at all,
  which a browser treats as a refusal. A page on this project's domain therefore cannot obtain a time stamp
  by itself, and the relay's time-stamping role is required, as the website decision assumed.

  The method was a preflight from outside a browser rather than a real page, which is a faithful stand-in:
  the headers a browser needs are the headers the authority returns, whoever asks.

  ```console
  $ curl -sS -D - -o /dev/null -X OPTIONS -H 'Origin: https://adiungere.com' \
      -H 'Access-Control-Request-Method: POST' -H 'Access-Control-Request-Headers: content-type' <authority>
  ```

## P26 What the forensic and legal sources actually say

- **Verdict:** not started
- **Milestone:** M3
- **Question:** is there a named judgment behind the claim that a court may not set aside dashcam video on
  suspicion alone, and what do the two forensic bodies say word for word about authentication conclusions?
- **Method:** read the primary sources and quote them here.
- **Decides:** every legal sentence on the website and in the report. Until it is answered, none is written.

## P27 Written licence opinion for the Linux media stack

- **Verdict:** reasoned
- **Milestone:** M1
- **Question:** what are the exact terms for dynamically linking the media framework from a sandboxed
  runtime, for the language bindings, and for the native window materials on Windows?
- **Method:** read the texts, write an opinion, record it here.
- **Decides:** the dependency policy exception and the desktop decision record.
- **Result:** reasoned on 2026-09-15 from the licence declarations of the crates as published in the
  registry and from the licence texts they name. This is the maintainer's reading, not legal advice, and
  it is recorded so that a person who disagrees can say with which sentence.

  **The media framework.** The framework's libraries are published under the GNU Lesser General Public
  License, version 2.1 or later. Section 6 of that licence permits a work that links to the library to be
  distributed under terms of the distributor's choosing, on the conditions that the work does not restrict
  reverse engineering for debugging, that the licence is reproduced, and that the person can relink the
  work with a modified library. Dynamic linking against a shared library the person can replace satisfies
  the relinking condition by construction. In the sandboxed distribution channel the framework lives in the
  runtime and its codec extension, outside this project's package, and the application binds to it at
  load time; the project distributes no copy of the library at all. The Apache-2.0 application is therefore
  a work that uses the library, its own terms unaffected, and the obligations are a licence notice in the
  package and no measure against relinking, both of which M5 carries. Static linking, or bundling the
  library inside the package, would be a different case and is not proposed.

  **The bindings.** The bindings crates for the toolkit and its adaptive layer are published under MIT; the
  bindings crates for the media framework under MIT or Apache-2.0 at the user's choice; the paintable sink
  plugin under the Mozilla Public License 2.0, which is file-level copyleft: it obliges the publication of
  changes to its own files and nothing about the files that link to it. All three are inside the dependency
  policy's allow list without exception. Nothing in the bindings carries the framework's licence: they are
  distinct works that call it.

  **The immediate-mode toolkit** considered for a fallback shell is published under MIT and raises no
  question. **The declarative toolkit** also considered is published under the GNU General Public License
  version 3 only, a royalty-free licence with its own conditions, or a commercial licence: none of the
  three is in the policy's allow list, so it is out, and the desktop decision record already says so.

  **The window materials on Windows.** The crate that applies the native window materials is published
  under Apache-2.0 or MIT, inside the allow list, and the materials themselves are functions of the
  operating system that any application may call.

  Consequence: the policy's allow list needs no new entry. The dependency policy carries one written
  exception for the media framework, stated as a runtime dependency outside the dependency graph, dynamically
  linked from the sandbox runtime, with the licence notice as the obligation; M5 writes the decision record
  and the note in the policy file together.

  ```console
  $ cargo info gtk4 libadwaita gstreamer gst-plugin-gtk4 iced slint window-vibrancy
  ```

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

- **Verdict:** measured
- **Milestone:** M1
- **Question:** what does the documented third-party command actually write, in particular where it inserts
  parameter sets, and is that stable between versions of the tool?
- **Method:** compare the output of two versions of the tool on the reference clip and specify the recipe.
- **Decides:** the definition of the secondary digest, and the pinned tool version.
- **Result:** measured on 2026-09-15 on the reference recording with two versions of the tool: a build
  from February 2026 on the maintainer's machine, and the build of 15 September 2026 that
  `tools/versions.toml` pins and the pipeline fetches. Both write byte-identical streams for both video
  tracks, on the reference recording and on every recording of the synthetic corpus, so the recipe below
  is stable across the versions measured and the pinned one is the definition.

  Copying a video track to a raw elementary stream applies the container-to-stream filter implicitly, and
  naming the filter explicitly produces a byte-identical file. The filter inserts the sequence and picture
  parameter sets before **every** key frame: on the reference recording, 120 key frames, 120 of each
  parameter set, 1680 other frames, no access unit delimiters, no supplemental enhancement messages. Start
  codes follow one rule: four bytes before each parameter set and before the first unit of every access
  unit, three bytes before the key frame slice that follows its parameter sets inside the same access unit.
  The stream therefore begins `00 00 00 01 67`, and every ordinary frame is one unit with a four-byte start
  code.

  That is the exact definition the secondary digest has to reproduce: for each access unit in decode order,
  if it is a key frame, the parameter sets from the decoder configuration record each with a four-byte start
  code, then the slice with a three-byte start code; otherwise the slice with a four-byte start code. The
  digests of the two tracks so produced, as a second implementation must match them:

  ```console
  $ ffmpeg -v error -i <recording> -map 0:v:0 -c copy -f h264 front.h264
  $ ffmpeg -v error -i <recording> -map 0:v:1 -c copy -f h264 rear.h264
  $ sha256sum front.h264 rear.h264
  6c2ab7086966364ed9326628a126241dc2040acfaf03d26ea148ee8cb674a891  front.h264
  c67a472790d4b818247850ece29db681701702725499a8b33d86a47aac5ba14b  rear.h264
  ```

  The front stream is 74 557 736 bytes and the rear 59 567 392. The product's own rule, `adiungere-annexb/1`
  in `docs/integrity.md`, reproduces both digests, and so does the reference script; guarantee **G14**
  keeps the three in agreement on every corpus recording on every pull request.

## P35 The real minimum compiler versions of the dependency set

- **Verdict:** measured
- **Milestone:** M1
- **Question:** what minimum compiler version does each dependency in the eventual set require, and is a
  single pin compatible with all of them?
- **Method:** read the manifests, compile against the candidate pin.
- **Decides:** the pinned toolchain and the lint configuration.
- **Result:** read from the registry on 2026-09-15, latest stable release of each. The highest declared
  minimum in the set is 1.92, for the Linux toolkit and media bindings; the provenance library declares
  1.88, the browser binding 1.77, the digest library and the argument parser 1.85, the two serialisation
  crates 1.56 and 1.71. Four crates declare no minimum: the foreign function binding generator, the box
  codec, the fuzzing harness and the Linux widget bindings. The pin of 1.98.1 satisfies every declared
  minimum with room, and the workspace minimum of 1.98 is what a contributor needs rather than what the
  dependencies need, which is the honest reading: this workspace uses features of its own pin.

  Licences read at the same time: every crate in the set is permissive and on the accepted list, with two to
  note. The foreign function binding generator is under the file-scoped copyleft the policy accepts and
  names. The fuzzing harness carries an additional university licence and lives in a separate project
  outside the workspace, so it never enters the shipped graph.

  ```console
  $ curl -sS https://crates.io/api/v1/crates/<crate> | jq '{max: .crate.max_stable_version, msrv: .versions[0].rust_version, licence: .versions[0].license}'
  ```

## P36 Whether the core builds everywhere it is promised

- **Verdict:** measured
- **Milestone:** M1
- **Question:** does the core, including the provenance library and its cryptography and transport stack,
  compile and pass its tests on Windows, both macOS architectures, a static Linux target and WebAssembly?
- **Method:** a build matrix, from the milestone that first has code worth building.
- **Decides:** the supported platform matrix and the feasibility of the browser export engine.
- **Result:** measured on 2026-09-15 for the two targets that decide the most, on a scratch crate that
  depends only on the provenance library. With the library's default features it does **not** compile for
  WebAssembly: the defaults select a system cryptography library that has no WebAssembly build. With the
  defaults off and the library's native-Rust cryptography feature on, it compiles for both the Linux target
  and WebAssembly, and no system cryptography crate remains in the graph of either. That configuration is
  the one the core adopts everywhere, so the same cryptographic code runs on every surface and no platform
  needs a C library.

  The resolved graph under that configuration is 308 crates.

  Extended on 2026-09-15 with the M1 crates. The reader, the fingerprints, the manifest and the scanner
  build for WebAssembly, and the whole workspace less the guarantee suite builds for static Linux, both
  measured locally with the pinned compiler. The pipeline now carries the full matrix: the gate sequence
  on Linux, build and tests on macOS on both architectures and on Windows, a static Linux build, and a
  WebAssembly build of the four browser crates. Windows and macOS are measured by that pipeline on the
  first push of M1, and this entry is extended with their answer.

  ```console
  $ cargo check --target wasm32-unknown-unknown
  $ cargo tree --target wasm32-unknown-unknown -e normal | grep -ciE 'openssl|ring|aws-lc'
  $ cargo build --target wasm32-unknown-unknown -p adiungere-isobmff -p adiungere-fingerprint \
        -p adiungere-manifest -p adiungere-scan
  $ cargo build --workspace --target x86_64-unknown-linux-musl --exclude adiungere-guarantees
  ```

## P37 The structure of the reference recording

- **Verdict:** measured
- **Milestone:** M1
- **Question:** how is a recording from this camera laid out, box by box, and what does it carry beyond
  the video and the audio?
- **Method:** read the box tree and the stream parameters with the container probe tool, and read the bytes
  of the vendor box.
- **Decides:** the synthetic fixture, which has to reproduce every property found here, and every sentence
  in the README that describes what the product preserves.
- **Result:** measured on 2026-09-09 on the reference recording, and confirmed on the two earlier
  recordings on 2026-09-15.

  The file type box declares the common brand with the earlier one as compatible. The movie box comes
  **first**, at about eighty kilobytes, followed by a top-level vendor box of twelve bytes whose four-byte
  payload is a model code, then a single media data box holding the rest. Three tracks: two video tracks in
  the same advanced-profile coding at 1920 by 1080 and thirty frames per second, 1800 samples each with a
  key frame every fifteen, no composition offsets, and both marked as default tracks, which is why players
  cannot agree which to show; one audio track in the usual low-complexity coding at 44.1 kHz. The decoder
  configuration of both video tracks declares four-byte length prefixes on every unit. The user-data box
  holds a vendor box of 30 720 bytes: a camera identity and firmware string, one satellite sentence per
  second for the duration, two further text record types at the same rate, and about twenty binary motion
  records of sixteen bytes per second. Nothing in the movie box beyond the user-data box is
  vendor-specific.

  An extraction made with the common command-line tool, for comparison, writes the media data box before
  the movie box, drops the user-data box and the top-level vendor box, and replaces the encoder tag. That is
  the outcome every export of this product is measured against not reproducing.

  Re-read on 2026-09-15 with this product's own reader, which the probe tool had left unsaid or coarse:
  the user-data box holds a **second** child, a 36-byte requirement atom of the older container family
  written by the recorder's firmware, so the manifest lists three vendor boxes and not two; each video
  track carries an edit list of one entry over the whole duration, the audio track none; every sample
  table holds padding boxes after its tables, three per video track and two for audio; the movie box is
  84 175 bytes and a structural inspection reads 84 271 bytes in all, never the media. The re-export's
  user-data box holds only a metadata box of 89 bytes carrying the other program's encoder tag, which the
  reader classes as a standard box rather than a recorder's.

  Every number the milestone's *Done when* names was then reproduced without this product: the whole-file
  digest with a digest utility, the three track fingerprints and their configuration digests with the
  reference script, the two elementary stream digests with the pinned media tool and a digest utility, the
  vendor box digests with a byte-range copy and a digest utility. The numbers are pinned in
  `core/fixtures/reference.json` and the nightly pipeline reproduces them from the private recording.

  ```console
  $ ffprobe -v error -show_format -show_streams <recording>
  $ ffprobe -v error -show_entries packet=pos,flags -select_streams v:0 <recording>
  $ adiungere inspect <recording>
  $ adiungere fingerprint <recording> --manifest <recording>.manifest.json
  $ scripts/reference-clip.sh check <recording>
  ```

## P38 What the host serves for a WebAssembly asset and how long it caches one

- **Verdict:** not started
- **Milestone:** M4
- **Question:** does the host return the WebAssembly media type for a `.wasm` file, and what cache lifetime
  does it set on an asset with a hashed name?
- **Method:** publish the first real asset of each kind with the site of M4 and read the response headers
  from outside.
- **Decides:** whether the browser can instantiate the core's WebAssembly build by streaming, and whether
  the site's assets need a lifetime set on the host or a rename on every change.

## P39 The clock burned into the picture against the other clocks

- **Verdict:** measured
- **Milestone:** M6
- **Question:** the recorder burns a date, a time, a speed and a position into the front picture. Does that
  clock agree with the file name, with the satellite sentences and with the container clock, does it run at
  the picture's rate, and what does the burned-in strip give away?
- **Method:** decode the first and the last picture of each video track of the reference recording with
  the pinned media tool and read the strip; compare with the file name, the first satellite sentence and
  the container clock read by `adiungere inspect`.
- **Decides:** whether the burned-in clock becomes a fifth clock of the manifest at M6, read by the
  platform decoder and a matcher for the recorder's own glyphs, or entered by the person who reads it;
  and what the masking of M6 has to cover beyond faces and plates.
- **Result:** measured on 2026-09-16 on the reference recording.

  The front track carries the strip, in the top-left corner, on every picture; the rear track carries
  none. The burned-in clock reads **one second before** the time in the file name on the first picture,
  and fifty-nine seconds later on the picture at fifty-nine seconds: it runs at the picture's rate. The
  first satellite sentence is two seconds after the file name, as P15 found. The container clock is
  twenty-five minutes before all of them, as P15 found, and a file manager that shows it as the media's
  creation time converts it from universal time on top of that, so the "properties" of the file show a
  time about an hour and a half away from the picture. Three clocks agree to within three seconds, one
  does not describe the recording at all, and the product says which is which.

  The strip also carries the position to six decimals, which is a few centimetres. A front picture
  therefore gives the position away by itself, before any telemetry is read: an extraction of the front
  camera carries it in every picture, and the masking of M6 has to offer to cover the strip as well as
  faces and plates. The speed in the strip is the recorder's, from the satellite receiver, and is the same
  quantity the telemetry records once per second.

  ```console
  $ ffmpeg -v error -i <recording> -map 0:v:0 -frames:v 1 first.png
  $ ffmpeg -v error -ss 59 -i <recording> -map 0:v:0 -frames:v 1 last.png
  $ adiungere inspect <recording>
  ```
