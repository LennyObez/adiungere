# Integrity

What this product proves about a recording, how each number is defined, and how a stranger reproduces
every one of them with ordinary tools and none of this code.

## What is proved, and what is not

| Statement | Provable | By what |
|---|---|---|
| The stored samples of this track are identical, byte for byte, to those the manifest records | Yes | the track fingerprint, `adiungere-track-fp/1`, equal on both sides; reproducible with the reference script below |
| This file is the one that was seen when the manifest was made, to the byte | Yes | the SHA-256 of the whole file, reproducible with any digest utility |
| The vendor boxes of this file are the bytes the manifest records | Yes | the SHA-256 of each vendor box, header included |
| This file was written by the same kind of recorder, laid out the same way | Yes, as a shape | the structural fingerprint, with its observations listed beside it |
| This file existed no later than a stated instant | Later milestone | a time stamp from an authority, which M3 adds; until then every time is a clock's claim with the clock named |
| The recording was not altered before it was seen | **No** | nothing in a file attests to that; the product reports the signs of a rewrite and says it is not an authentication examination |
| What the pictures show happened | **No** | outside the reach of any tool |

Every provable statement is a fact about bytes. The product forms sentences about those facts from a
catalogue, and a guarantee refuses any sentence that reads as a verdict.

## Definitions

### `adiungere-track-fp/1`, the track fingerprint

For one track of an ISO base media file:

1. Locate every sample in decode order, by the sample tables: the sample-to-chunk runs, the chunk offsets
   (32-bit or 64-bit), the sample sizes (constant, per-sample, or compact) and the time-to-sample runs. The
   tables are cross-checked; a track whose tables contradict each other has no fingerprint.
2. Read each sample's stored bytes, exactly as they lie in the file. For video this includes the length
   prefix before each unit; nothing is converted.
3. The **payload digest** is the SHA-256 of those bytes concatenated in decode order, with nothing between
   them.
4. The **configuration digest** is the SHA-256 of the payload of the decoder configuration box inside the
   first sample entry: the `avcC` box for advanced video, the `esds` box for audio, each without its
   eight-byte header and, for a full box, with its version and flags.
5. The record carries the definition's name, the track's index and identifier, the sample count, the byte
   count, the width of the length prefix when the configuration declares one, and the two digests.

The fingerprint is invariant under everything that rewrites the container around unchanged samples: moving
the movie box, regenerating the tables, adding or removing an edit list, adding a manifest. It changes when
a sample changes, when the configuration changes, and when the length prefix width changes, because the
prefix is stored with the sample. Two files holding the same pictures under different prefix widths have
different fingerprints and both are correct: the fingerprint is a claim about bytes, not about pictures.

### `adiungere-annexb/1`, the elementary stream digest

The secondary digest, for advanced video tracks only, defined so that the common command-line media tool
reproduces it without any code from this project. It is the SHA-256 of the track in start-coded form, built
as follows and measured against the tool in probe P34 at two versions:

1. Samples are taken in decode order and split into units by their length prefixes.
2. A unit that opens a picture's output gets the four-byte start code `00 00 00 01`; every later unit of the
   same picture gets the three-byte code `00 00 01`. Inserted parameter sets count as output.
3. Before the first refresh slice (unit type 5) of a refresh picture, unless a sequence or picture parameter
   set already appeared in the same sample, every sequence parameter set and then every picture parameter
   set from the decoder configuration is written, each with a four-byte start code. If only a sequence
   parameter set appeared in the sample, the picture parameter sets alone are written.
4. A refresh picture is "new" at the start of the stream, after any parameter set unit, and at any refresh
   slice whose first macroblock is zero once a previous refresh picture has had its insertion.

The stream digest does not depend on the length prefix width, and it is never equal to the track
fingerprint: the two digest different byte strings. Both are published; neither is presented as the other.

### The file digest

The SHA-256 of every byte of the file. An original is never rewritten by this product, so this is the
number that proves a file is the one that was seen.

### The structural fingerprint

The SHA-256 of a canonical listing of the container's shape: the brand and compatible brands, the order of
the top-level boxes, every unknown top-level box with its size, the movie timescale, the children of the
movie box, every child of the user-data box with its size, and for each track its kind, its sample entry
type and children, its compressor name, its handler name, its flags, its timescale, and the order of the
boxes under the track and under the sample table. The observations are recorded beside the digest so a
reader sees what was compared.

Two recordings from one device share a structural fingerprint. A file written from one of them by another
program does not, and the observations say where it differs.

### Times

A time in a manifest is a value, a clock from a closed set, and a note on what that clock is worth:

| Clock | What it is |
|---|---|
| `container_clock` | the creation time in the movie header; written by the recorder into a field defined as universal time, which the reference recorder fills with a local session clock shared by consecutive files |
| `file_name` | the local time in the file name, under a naming grammar, with no zone |
| `satellite_clock` | the universal time in the satellite sentences of the vendor telemetry; read at M6 |
| `system_clock_at_manifest` | the clock of the machine that produced the manifest, read as universal time |

Every sentence the product forms about a time says "no later than" and names the clock. None of these
clocks is taken as the capture time; when they disagree, the disagreement is stated.

## Reproducing every number without this code

For a recording `clip.mp4` and the manifest the product wrote for it:

```console
$ sha256sum clip.mp4
$ python3 scripts/track-fingerprint.py clip.mp4 0 --annexb
$ ffmpeg -v error -i clip.mp4 -map 0:v:0 -c copy -f h264 - | sha256sum
```

The first line reproduces the file digest. The second reproduces the track fingerprint of track 0, payload
and configuration, and the elementary stream digest, from nothing but an interpreter's standard library.
The third reproduces the elementary stream digest of the first video track with the media tool, at the
version pinned in `tools/versions.toml`; `-map 0:v:1` is the second video track. The reference script is
[`scripts/track-fingerprint.py`](../scripts/track-fingerprint.py): about a hundred lines, of which the
fingerprint itself is forty, written to be read.

Guarantee **G14** runs both reproductions on every recording of the synthetic corpus on every pull request,
and the nightly pipeline runs the product on the reference recording and compares every number with
[`core/fixtures/reference.json`](../core/fixtures/reference.json).

## The manifest

`adiungere-manifest/1` is one JSON document: how it was produced, the file and its structure, every track
with its fingerprints, every vendor box with its digest, and every time with its clock. Its schema is
published at
[`core/crates/adiungere-manifest/schema/adiungere-manifest-1.json`](../core/crates/adiungere-manifest/schema/adiungere-manifest-1.json)
and a test keeps
it equal to the one the types derive. Unknown fields are refused on reading, so a manifest from a later
version is refused rather than half-read.

The canonical text form is what `adiungere fingerprint --format json` prints: two-space indentation, the
fields in the order the schema lists them, digests in lower-case hexadecimal, a trailing newline.

Two fields describe what a general-purpose reader would have hidden. A vendor box carries `standard`: true
when its type is one the base media file format or its 3GPP extension defines under the user-data box,
such as the metadata box a common muxer writes, and false for a recorder's own box. Every box is
preserved and digested either way; only the sentence that says whether the recorder's boxes are present
counts the second kind. The structure carries `clipped_tail_bytes`: how many bytes at the end of the file
do not read as a box or belong to a box that declares more than the file holds, as a recorder leaves them
when it pre-allocates a file or loses power, and nothing when the file ends with a well-formed box. Such a
file is inspected in full; a fingerprint over samples the file no longer holds is refused with the range
that was asked for.

### The operation

A manifest says what produced it. An inspection carries its scope: the structure alone, or the whole file
with every digest. An export carries three things, permanently and distinctly: its **class**, its
**masking**, and its **sources**.

The class is one of four words, and the sentence a surface shows about an output derives from this word
and from nothing else. An `extraction` copies the samples of one camera and the audio byte for byte into
a new container. A `two_track_archive` copies both cameras and the audio into one file with two video
tracks, as the recorder wrote them, whether they came from one recording or from the two files of a
recorder that writes one per camera. A `rendition_lossy` is a composition that went through an encoder,
and a `rendition_lossless_verified` is a composition whose decoded pictures were compared with the source's
and found equal, a label that is never given before that comparison has run. The first two are stream
copies and carry the source's fingerprints; the last two carry the source's fingerprints as ingredients
and never the sentence that says a track is identical to its source.

The masking is `none`, `faces_and_plates`, or `faces_plates_and_strip`, the last covering the position and
clock the recorder burns into the picture. It is a choice a person makes at export and never a default: a
file for a claim or a court keeps faces and plates, because the reader needs them and a masked file is a
re-encoded composition that is no longer the evidence; a file for publication is masked. An extraction and
a two-track archive always carry `none`, since a masked output is by definition re-encoded.

Each source is named by its file name, its size, its whole-file digest, and the tracks taken from it, each
with the index it had in the source, the index it has in the output, and the fingerprint it had in the
source. `adiungere export` reads its output back and refuses to keep it unless every track's fingerprint
in the output equals the fingerprint recorded here; a refused output is removed.

```console
$ adiungere export <recording> --camera rear --out rear.mp4
$ adiungere fingerprint rear.mp4 --format json | jq '.tracks[].fingerprint.payload_sha256'
$ jq '.produced.operation.sources[].tracks[].fingerprint.payload_sha256' rear.mp4.manifest.json
```

The two lists are equal, track for track, and the second one is what the source recording's own
`fingerprint` prints for the tracks that were taken. A third party checks the extraction the same way,
with the reference script of this document on both files.

## Comparing a manifest with a file

`adiungere verify manifest.json clip.mp4` compares subject by subject: the whole file, each track, each
vendor box, the structure. Each subject has one of six outcomes: identical, differs, not recorded, missing
from the file, present but not recorded, or not checkable with the reason. The command's status is 0 when
every compared subject is identical and 1 otherwise. There is no summary line that says the recording is
fine, because the findings are the answer and the reader draws the conclusion.

A whole file that differs while every track is identical is a container rewritten around unchanged
samples. A track that differs is a sample that changed. A vendor box that is missing is what every
general-purpose rewrite does to a recording. Guarantee **G18** holds that one flipped byte produces exactly
the finding it belongs to.

## The wording

Every sentence the product forms lives in
[`core/crates/adiungere-manifest/wording/en.json`](../core/crates/adiungere-manifest/wording/en.json),
and every word that would turn a fact into a verdict lives beside it in
[`en.forbidden.json`](../core/crates/adiungere-manifest/wording/en.forbidden.json). Guarantee **G16**
refuses any string that contains a forbidden word as a whole word. The catalogue is the single source for
translation; a language is added by adding its two files.
