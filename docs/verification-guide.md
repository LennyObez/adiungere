# Verification guide

For the driver who recorded it, the claims handler who receives it, and the lawyer who will have to say
what it is. This guide says what each file adiungere produces means, what can be checked without adiungere
and how, and what none of it establishes. Every sentence below about what is proved has a check behind it;
where there is no check, the guide says so rather than filling the gap with a word.

## What you may be holding

| File | What it is | Who made it |
|---|---|---|
| `20260604_122323E.MP4` (the name is the recorder's) | A recording as the camera wrote it: two video tracks and one audio track in one file, with the camera's own boxes inside | the camera |
| `rear.mp4`, `front.mp4`, `both.mp4` | An export: the coded samples of one camera, or both, copied byte for byte into a new container, with the camera's boxes carried across | adiungere |
| `rear.mp4.manifest.json` | The adiungere manifest of that file: its digest, the digest of every track's samples and of every camera box, the recordings it came from and their digests | adiungere |
| `20260604_122323E.c2pa` | Content Credentials for the recording, beside it: a signed statement of the same facts, the recording untouched | adiungere, signed with a credential |
| `rear.mp4` with credentials embedded | The export with the signed statement inside the file, bound to the file's bytes | adiungere, signed with a credential |
| `rear.mp4.manifest.json.tsr` | A time-stamp token from a time-stamping authority over the manifest | the authority |

Nothing adiungere makes replaces the recording. The recording is the evidence; everything else describes it,
and each description can be checked against it.

## What each file establishes, and by what

| Statement | Established by | Check it with |
|---|---|---|
| The samples of track 1 of `rear.mp4` are, byte for byte, the samples of track 2 of the recording | the track fingerprint in the manifest, equal in both files | the reference script on both files, or `adiungere verify` |
| `rear.mp4` is, to the byte, the file the manifest describes | the whole-file digest in the manifest | `sha256sum rear.mp4` |
| The camera's boxes in the export are the camera's boxes in the recording | the digest of each box in the manifest | `adiungere verify` |
| These facts were signed by the holder of this credential, over this file, and the file has not changed since | the Content Credentials and the validator's result | the pinned validator, or `adiungere verify` |
| The manifest existed no later than this instant, by the authority's clock | the time-stamp token | the certificate tool of your platform, or `adiungere verify` |
| The recording was not altered before adiungere saw it | **nothing** | nothing; see below |
| What is on the picture happened as it shows | **nothing** | nothing; outside any tool |

## Checking without adiungere

A claim that only its author's tool can check is a claim about the tool. Every number can be reproduced with
tools that are not this product; the commands are in [`integrity.md`](integrity.md) and `adiungere report`
prints them for the manifest in hand. In short:

```console
$ sha256sum rear.mp4
$ python3 scripts/track-fingerprint.py rear.mp4 0
$ ffmpeg -v error -i rear.mp4 -map 0:v:0 -c copy -f h264 - | sha256sum
```

The first line reproduces the whole-file digest in the manifest. The second reproduces the track fingerprint
with a script of forty lines that uses the interpreter's standard library and nothing else. The third
reproduces the elementary stream digest with the common media tool, at the version named in the manifest's
documentation.

For the Content Credentials, the validator published by the maintainers of the standard reads a signed file,
or a recording with its `.c2pa` file beside it, and prints what it found:

```console
$ c2patool rear.mp4
$ c2patool 20260604_122323E.MP4
```

Read two things in its answer: `validation_state`, and the list under `failure`. `Valid` with the single
failure `signingCredential.untrusted` means the signature and the binding hold and the signer's certificate
is not on the standard's trust list, which is the state of every signature adiungere makes until it signs
with a credential from a listed authority. `Invalid` with `assertion.bmffHash.mismatch` means the file
changed after it was signed. The assertion labelled `com.adiungere.integrity` inside the answer is the
adiungere manifest itself, as it was signed.

For the time-stamp token, the certificate tool of your platform reads it:

```console
$ openssl ts -reply -in rear.mp4.manifest.json.tsr -text
$ openssl ts -verify -data rear.mp4.manifest.json -in rear.mp4.manifest.json.tsr -CAfile <the authority's chain>
```

The first prints the time the authority attested and the digest it attested it over; the second checks the
token against the manifest and the authority's chain, which the authority publishes.

## Checking with adiungere

```console
$ adiungere verify rear.mp4.manifest.json rear.mp4
```

The answer has three parts, and each line of it is a fact about bytes:

- **The comparison**, subject by subject: the whole file, each track, each camera box, the structure. Each
  is identical, differs, was not recorded, is missing, or could not be checked, and the answer says which.
- **The credentials**, when the file carries any: who signed, with what algorithm; whether the signature
  and the binding hold; whether the signer's certificate chains to the trust list or to none; the time the
  authority attested, if one was asked; the actions and the sources the signed statement names; and whether
  the signed facts carry the fingerprints this file has.
- **The token**, when one is beside the manifest: the time it attests, and that it names this manifest.

The command's status is 0 when every compared subject is identical, the credentials hold and the token
names the manifest, and 1 otherwise. The status is not an opinion about the recording. It says whether the
files agree with each other.

## What none of this establishes

adiungere performs no authentication examination, and its wording never says it did. The forensic bodies
that define such examinations describe them as work an examiner does, with conclusions an examiner forms.
The Scientific Working Group on Digital Evidence, in *Best Practices for Digital Video Authentication*,
23-V-001, version 1.2 of March 7, 2024, defines the field and bounds its conclusions:

> Authentication is defined as the process of substantiating that the data is an accurate representation
> of what it purports to be. (section 1)

> It is possible for the results of an examination to be: Consistent with an original; Inconsistent with an
> original; Inconclusive. (section 8)

> Language implying absolute certainty should be avoided unless discussing known alterations or deletions.
> (section 8)

> Metadata cannot be relied upon in isolation and should be used in conjunction with other elements of the
> file when possible. (section 5.3)

The European Network of Forensic Science Institutes, in its *Best Practice Manual for Digital Image
Authentication*, ENFSI-BPM-DI-03, issue 01 of October 2021, states the same for images:

> The final conclusion of an authentication examination states the evidential weight of (all) the findings
> as a level of support for one of the competing propositions. (section 12.2)

> Hash value: the output string produced by a hashing function [...] It is commonly used as a means for
> verification that the input data has not changed from the point in time that the hash was first
> calculated. (section 3, definitions and terms)

That last sentence is the whole of what adiungere's digests claim: that the data has not changed since the
digest was first calculated. A digest says nothing about what happened before it was taken. adiungere
takes its first digest when it first sees a file, which is after the camera wrote it and after every
gallery, transfer and re-export in between. That is why the product says "no sign of rewriting was found in
the container structure" and never "not altered", and why its one absolute sentence is the equality of a
digest with a digest recorded earlier.

A signature adds a name and a time, not truth. It establishes that the holder of a credential put their
name to these facts at an instant an authority attested. It establishes nothing about whether the facts
describe what the camera saw. A credential adiungere generates for a run is on no trust list, and every
surface says so; a credential from a listed authority attests the signer's identity to the standard's
degree, and no more than that.

## For the person who has to decide

Ask for four things: the recording itself, its manifest, the exports that were shown with their manifests,
and, if any were made, the Content Credentials files and the time-stamp tokens. Check that the manifests
describe the files in hand with `adiungere verify` or with the commands above. Read the three-word state of
any credentials as the validator returns it, not as a badge. Treat a time stamp as the authority's word,
with whatever standing that authority has where you are; a time stamp from a qualified authority under the
applicable electronic-signature regulation carries a presumption there that another does not, and this
product does not say which yours is.

Then ask the questions no tool answers: who had the recording between the camera and you, and what did they
do with it. The manifests tell you what a file is. They do not tell you where it has been.
