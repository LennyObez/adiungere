# Roadmap

Eleven milestones. Each one produces something usable and ends with a named demonstration; none ends on an
invisible layer. Each becomes a GitHub milestone, each milestone becomes an epic, and each task below becomes
an issue.

The order is not negotiable in one respect: **M4 is the first real product**. Everything before it exists to
make M4 possible, and everything after it widens a loop that already works. The plan can stop after any
milestone without leaving a layer nobody can see.

A milestone produces **one signed commit**. The pull requests that led to it stay as the review record.

---

## M0: Foundation

*Done when a pull request that deliberately violates each guarantee turns the corresponding gate red, the
repository shows the settings, ruleset, labels, milestones, epics and project this list declares, a static
placeholder page is served over HTTPS at adiungere.com, and the first signed commit lands on main with
authorisation.*

- [x] Repository `LennyObez/adiungere`, public, discussions on, wiki off, squash and rebase only, merged
      branches deleted
- [x] Apache-2.0 with `NOTICE`, no contributor agreement, inbound equals outbound stated in `CONTRIBUTING.md`
- [x] Health files, `.editorconfig`, `.gitattributes`, `.gitignore`
- [x] Declared label set applied from the repository, dependency updates for cargo and actions, issue forms
      for a bug, a feature and a probe, pull request template, code owners
- [x] Ruleset on the default branch: deletion, non fast-forward, linear history, signed commits, pull request
      with thread resolution, squash or rebase only, required checks named, no bypass
- [x] Milestones M0 to M10, one epic per milestone, public project with the field set the project's own
      description explains
- [x] Roadmap, architecture, testing, getting started, evidence register, decision record template and the
      founding records, each with the check that enforces it
- [x] Placeholder README in every directory that is empty until a later milestone
- [x] Rust workspace with the pinned toolchain, workspace lints, dependency policy and cargo aliases
- [x] Evidence register readable as data: `adiungere probes` lists it, shows one entry and refuses a register
      that disagrees with this roadmap
- [x] Guarantee suite for the M0 properties, each watched failing against a deliberate violation before
      being trusted
- [x] Continuous integration: a repository-wide workflow with no path filter, a core workflow, a site
      workflow, the label synchroniser, and code scanning over both the Rust and the workflow languages, with
      every action pinned to a commit and tokens read-only by default
- [x] Static site with a security contact at the well-known path, deployed from the default branch
- [x] Probe P19 recorded: what the production host serves, and how a deployment reaches it
- [x] Citation metadata, and the decision not to publish a funding file recorded

Two of these lines closed with the M1 commit rather than the M0 one, and the roadmap said so in between: the
site went live after the M0 commit was published, and the probe that measures the host could only be
answered once it was. A commit already published is never rewritten to make a milestone look complete
earlier than it was.

Deferred on purpose, with the reason written where it belongs rather than hidden: **the external tool version
pins** (`tools/versions.toml`) arrive in M1 with the first oracle that reads them, because a pin file nothing
reads is a file nobody maintains.

## M1: Inspection and fingerprints

*Done when adiungere inspect, fingerprint, detect and verify on the reference clip produce a manifest whose
every number a third party reproduces with sha256sum, the published reference script and the ffmpeg Annex B
recipe, the same commands on a moov-last re-export report equal fingerprints and the vendor boxes as absent, a
moov-only inspection of the reference clip reads fewer than 256 KiB, the required checks pass on the public
synthetic fixture alone, and every probe listed here has a verdict in the evidence register.*

- [x] Box reader: opaque ranges by default, typed views only where a value is needed, `udta` and unknown
      boxes as ranges, sample description entries copied as ranges
- [x] Sample iterator in decode order, yielding stored bytes
- [x] Fingerprints: track fingerprint version 1, decoder configuration digest, Annex B secondary digest,
      whole-file digest, writer structural fingerprint
- [x] Integrity specification with a reference implementation and the third-party commands that reproduce it
- [x] Manifest types with unknown fields refused, a published schema, and graded verification
- [x] Scanner: naming grammars by shape, bounded moov probe, paired-file sources, derivative detector
- [x] Command line: inspect, fingerprint, detect, verify, report, with machine and human output
- [x] Public synthetic fixture carrying every property the reference clip has, and the private clip pinned by
      digest and used only in the nightly pipeline
- [x] External tool version pins, read by the pipeline rather than written in it
- [x] Fuzzing on a dated nightly, with mutation testing and Miri
- [x] Probes P01, P02, P03, P15, P25, P27, P34, P35, P36 and P37 recorded, with P15, P25, P34, P35, P36 and
      P37 measured ahead of the code they inform
- [x] Core platform matrix, including a WebAssembly build

Two probes carry a half that only the pipeline can measure, and the evidence register records each: P01's
Apple reader ran on the macOS runner and P36's Windows and macOS builds ran in the matrix on the
milestone's pull request, which is where a maintainer's machine stops. The nightly pipeline is written,
watched failing where a guarantee reads it, and first executed on the default branch after this milestone
lands.

## M2: Lossless extraction

*Done when front-only, rear-only and two-track exports of the reference clip carry the vendor telemetry box
and the unknown top-level box byte for byte, moov before mdat, per-track fingerprints equal to the source,
byte-identical output on Linux, macOS and Windows, an independent parser reads the same sample counts, and the
rear-only file plays in the gallery applications of at least Windows and Android.*

- [x] Two-pass writer, moov first, deterministic interleaving, promotion to 64-bit offsets, streaming output
- [x] Sample tables rebuilt for a subset of tracks, edit lists kept
- [x] Vendor and unknown boxes copied as ranges, verbatim and at the same position relative to the movie
      box, because P02 measured no absolute offset in the telemetry payload
- [x] Two-track archive with no encoder, including joining two paired files into one two-track file
- [x] Export command with a manifest and a remux report, progress and cancellation
- [x] Golden tests per mode, property tests over the corpus, independent parsers as oracles
- [x] Architecture tests: no typed vendor container, no encoder symbol in the released binary
- [ ] The rear-only export played in the gallery applications of Windows and Android, recorded as P40 with
      the device, the version and the date, because that part of the demonstration is outside any pipeline.
      Windows and an iPhone are recorded; the Android gallery waits for a device

The writer copies the recorder's boxes as the ranges the reader found them in and rebuilds only the chunk
offset tables, so the sample description entries, the edit lists and the vendor boxes reach the output as
the bytes they were. The export refuses its own output when a track read back does not carry its source's
fingerprint, and removes it. The demonstration of this milestone has two halves: the reference recording's
rear-only export carried the telemetry box and the model code byte for byte, decoded without a complaint
under the pinned media tool with the same packet counts, and verified identical on every subject; the
gallery half is a person playing that file, recorded in P40 for Windows and an iPhone, and open for Android
until a device is at hand.

## M3: Provenance

*Done when an export carries an embedded Content Credentials manifest with the opened and repackaged actions,
the integrity assertion and a time stamp that an external validator accepts and the public check page displays
as an unknown signer, an original receives a sidecar manifest without one byte changing, and a faststart
applied after signing is watched invalidating the hard binding.*

- [ ] Provenance crate: parent ingredient, actions, integrity assertion, embedded and sidecar placement
- [ ] Sign-last pipeline, automatic re-signing, and the invalidation test that proves the order matters
- [ ] Time stamping against a development authority, with the production authority configurable
- [ ] Split signing flow measured, self-signed chain labelled as not on the trust list, certificate
      enrolment opened as a long-lead issue
- [ ] Verification extended to Content Credentials with an honest trust state, and the verification guide
- [ ] Decision record stating what a relayed signature proves and what it does not
- [ ] Probes P04, P05, P11 and P26 recorded, with the legal wording read in primary sources before any of it
      reaches the guide

## M4: Website and shared player, the first product

*Done when a person drops a folder on adiungere.com, sees every dual-track clip found, plays front, rear or
both with the single audio track, exports front, rear, the two-track archive losslessly and a composed
rendition labelled as re-encoded, with a manifest and a signed sidecar, and checks a file or manifest on the
check page, with footage never leaving the device, on current Chrome, Firefox, Safari and iOS Safari.*

- [ ] Typography research per platform, folded into the brand brief before it is sent
- [ ] Design briefs in dependency order, tokens in the repository with contrast and status tests
- [ ] WebAssembly surface with a segmented reader and writer
- [ ] Ingestion, library grid, first-run explanation, per-card timestamp with its source, paired-file
      playback as a first-class mode
- [ ] Three-tier player, capability probe before any claim, one timeline, full keyboard operation, an
      alternative to every drag, reduced-motion variants
- [ ] String catalogues in English and French, with the forbidden-verdict test applied to both
- [ ] The product name reaching every rendered surface from one value, with the decision record and the
      guarantee that proves a rename leaves no trace
- [ ] Export dialogue carrying the four export classes and the preservation checklist
- [ ] Composition through the platform encoder, finalised by the core, with the pixel-exact label awarded
      only after comparison
- [ ] Check page recomputing everything client-side
- [ ] Signing and time-stamping relay behind the host, with its data statement and its operating runbook
- [ ] Site: landing, application, check, documentation, installable, cache and security headers
- [ ] Browser tests on three engines with a network assertion
- [ ] Probes P06, P07, P08, P10, P12, P21, P28, P29, P32 and P38 recorded

## M5: Desktop for Windows and Linux

*Done when the application installs from the signed setup on a clean Windows 11 machine and from a clean
Flatpak on stock Fedora and Ubuntu, scans chosen folders and a memory card, plays the reference clip in every
view mode, exports every class and verifies through the in-process core, the Linux build plays inside the
sandbox with no host codec installed, and both installers carry attestations.*

- [ ] Windows shell hosting the shared player, core in process, native window materials
- [ ] Windows media path validated on an edition without the media feature pack, installer, package manifest
- [ ] Linux shell in GTK 4 over a two-branch media pipeline, with the sink probes run before any shell work
- [ ] Flatpak manifest and submission, with the codec extension declared and its absence reported clearly
- [ ] Desktop decision record with the upgrade path, the fallback path and the dynamic-linking exception
- [ ] Reusable release workflow with artefact attestations and the verification command in the README
- [ ] One script writes the version everywhere, and an independent check asserts they agree
- [ ] Probes P09, P17, P22, P23, P24 and P31 recorded, with P23 and P24 run before any shell work begins

## M6: Telemetry and privacy masking

*Done when satellite fixes and motion records render as a route, speed, heading and a motion strip, the
recording clock is shown beside the container clock with their disagreement stated, sidecars accompany every
export, the parser degrades to bytes preserved on any unknown layout, face and plate masking runs on device as
a composed rendition, no share affordance is reachable without it, and the telemetry format is published.*

- [ ] Telemetry parser derived from at least two clips, versioned by firmware string
- [ ] Route and motion sidecars, overlay off by default and burned in only on an explicit choice
- [ ] Map and graph components shared by the website and the desktop shells
- [ ] On-device masking, with the detection library settled by its probe or the manual fallback shipped and
      named as such
- [ ] Masking as an explicit choice at export, never a default: a file for a claim keeps every face and
      plate, because an insurer or a court needs them and a masked file is a re-encoded rendition that is
      no longer the evidence; a file for publishing is masked. The choice is named in the manifest.
- [ ] Share gate: sharing passes through masking or an explicit acknowledgement that the file is unmasked
- [ ] Burned-in clock read as a fifth clock beside the others, and the burned-in position strip offered to
      the masking step, as probe P39 measured
- [ ] Probes P33 and P39 recorded, since automatic masking depends entirely on the first and the clock and
      the strip on the second

## M7: Android

*Done when the application is in internal testing and the store-free flavour builds reproducibly with no
proprietary services, with picker and document-tree ingest, explicit track selection, the dual view behind
its flag measured on a mid-range device, core exports of every class written to the media store, the masking
step before any share, and the design tokens rendered as the platform theme.*

- [ ] Foreign function surface for Kotlin, packaged as a library archive, bindings committed with a
      freshness check
- [ ] Platform design brief, the four screens, picture in picture
- [ ] Photo picker by default, persisted document trees, the broad media permission declared only if the
      store approves it
- [ ] Player with an explicit track override, dual view behind a flag with two measured backends
- [ ] Exports through the core, composition through the platform transformer with a capability ladder
- [ ] Screenshot tests on every pull request, instrumented tests nightly
- [ ] Probes P14, P16, P18 and P30 recorded, with no claim about a cloud library made before P14 answers

## M8: Apple

*Done when the iOS build is in external testing and the macOS build is notarised, both from continuous
integration on free runners, with the two-tier photo library scan handling cloud-only and limited access, the
composition player in every view mode, core exports of every class, the masking step before any share, and
the platform chrome passing its reduced-transparency fallback.*

- [ ] Developer programme membership, certificates and profiles held as pipeline secrets
- [ ] Build script and root package manifest with a switch between a local and a released binary framework
- [ ] Platform design brief, the multiplatform target, the media actor
- [ ] Photo library scan with a bounded metadata probe and cancellation, cloud consent, limited access
- [ ] Composition player, deployment target settled by a decision record, dual view budget measured
- [ ] Exports through the core, verification screen, masking
- [ ] Probe P13 recorded, since writing back to the photo library depends on its answer

## M9: Evidence tier and sharing

*Done when an export signed with a certificate from a conformant authority validates on a third-party
verifier without an unknown-signer warning, carries a qualified time stamp from a European trust service
provider, the practitioner read-through of the verification guide is recorded and its remarks absorbed, and a
printable verification report exists for a claim file.*

- [ ] Conformant certificate and trust list enrolment, production signing configuration
- [ ] Qualified time stamping, batching design recorded, public ledger anchoring reassessed
- [ ] Practitioner read-through recorded as probe P20, with the documents corrected claim by claim
- [ ] Printable verification report, check page updated

## M10: Opening

*Done when an external audit of the integrity claims and the supply chain passes, the open-source best
practices badge is held, the name has cleared a trademark search in the relevant classes, and the ledger of
guarantees holds no committed row, only enforced ones.*

- [ ] External audit of the fingerprint specification, the wording and the release provenance
- [ ] Trademark clearance, with renaming exercised through the single configuration value if needed
- [ ] Supply-chain scorecard published with its score explained, best practices badge held
- [ ] Every committed guarantee moved to the enforced table, with the test that proves it
