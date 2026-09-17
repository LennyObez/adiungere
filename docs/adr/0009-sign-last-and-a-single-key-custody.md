# ADR-0009: Sign last, and keep one custodian for the key

## Status

Accepted

## Context

Embedding a provenance manifest into a file of this format inserts a box near the start and shifts everything
after it. The library therefore corrects the chunk offset tables as it writes. Those same offset tables are
**inside the default hard binding**, which is the hash the manifest is bound to.

The consequence is exact and unforgiving: any rewrite of the container after signing invalidates the
signature. Moving the movie box to the front for progressive playback is such a rewrite. So the order is not a
preference. Faststart, then sign. Never the other way round.

The second question is where the key lives. The product runs on six surfaces, four of which are downloadable
binaries, and a key inside a downloadable binary is extractable by anyone who takes the trouble.

## Decision drivers

1. A signature that a rewrite can silently invalidate is worse than none, because it fails at verification
   time in front of the person who needed it.
2. A key that can be extracted from a shipped application signs nothing.
3. What a signature actually proves must be stated by the product, not inferred by the reader.

## Decision

**Signing is the last step.** The container is finalised, the movie box is placed first, and only then is the
manifest embedded and signed. Any later modification triggers re-signing, and a test proves that a rewrite
after signing invalidates the binding.

**Originals are never touched.** A recording that the product did not produce receives an external manifest
alongside it, and the original file keeps its digest to the byte.

**One custodian holds the key: the service.** On every surface, including the native ones, manifests carrying
this project's identity are signed by the service, which rebuilds the claim from structured facts it receives
and never sees the media.

**What that signature proves is stated plainly.** The binding and the fingerprints are computed on the device.
The service signs a claim rebuilt from those facts **without being able to recompute them**, because it never
sees the file. So the signature attests that these facts were presented to the service at that instant, and
nothing more. The wording says so, and the manifest carries a signing state of signed, pending or unsigned.
There is no device signing context, because there is no device key.

Offline, a surface produces the fingerprints and the manifest, queues the signature, and displays that it is
not yet signed.

## Alternatives considered

### Sign first, then optimise the container for progressive playback

It is the order a naive pipeline produces, and it yields a file whose signature fails to validate. The
failure is discovered by the recipient, not by the producer.

### Exclude the offset tables from the binding

The specification's exclusions make this technically expressible. It would mean signing a file while
deliberately not covering the tables that say where its media is, which weakens exactly the binding the
signature exists to provide.

### A key on each device

The signature would be made where the file is, which is intuitively stronger. Every copy of the application
carries the key, so the signature proves possession of the application rather than anything about the file.

## Consequences

### Positive

- A signed export validates in external verifiers, because it was signed in the state it will be read in.
- No extractable credential ships anywhere.
- The claim the product makes about its own signature is narrow enough to survive a hostile reading.

### Negative

- Signing requires a network round trip, so offline exports are unsigned until the device is online.
- The service becomes a dependency of a feature that people will expect to work offline.
- The split signing flow is the heaviest unknown in the plan. It has no degraded mode that keeps the
  advertised product, only the honest fallback of a timestamped manifest with no Content Credentials.

### Neutral

- Until a certificate from a conformant authority is enrolled, exports are labelled as signed by an identity
  that is not on the trust list, everywhere and without exception.

## Enforcement

From M3, guarantee **G27**: signing is the last step, proved by applying a rewrite after signing and watching
the binding fail, then re-signing and watching it recover.

From M3, guarantee **G28**: an original that receives an external manifest is byte-identical to the original.

From M3, guarantee **G30**: the displayed signer identity equals the certificate subject, and the trust state
equals the validator's result.

## What would change this decision

Probe **P11** finding that the split flow does not exist. The fallback is already written: manifests and
fingerprints carry the proof independently of any provenance format, and the wording says exports carry no
Content Credentials.

A regulated client requiring an operator attestation would need a different custody model, which is a separate
tier with its own record.

## Security impact

The service is the only key custodian and therefore the highest-value target in the system. It signs only
claims it has rebuilt from structured facts, never opaque input, so a caller cannot ask it to sign arbitrary
bytes. Key rotation and revocation are part of its operating runbook rather than an afterthought.

## Privacy impact

The service sees digests and structured facts, never media. What it necessarily sees, an address and a time,
is stated on the privacy page with its retention.

## Performance impact

One network round trip per signed export, on a payload of a few kilobytes.

## Migration and rollback plan

Adoption is M3 for the pipeline and M4 for the service. Rollback is the unsigned path, which exists from the
first day precisely so that it is exercised rather than theoretical.

## Links

- [`docs/evidence.md`](../evidence.md), probes P04, P05, P11, P12, P17 and P25
- [ADR-0005](0005-the-product-never-returns-a-verdict.md),
  [ADR-0008](0008-the-website-runs-in-the-browser.md)
- [ADR-0015](0015-what-a-relayed-signature-proves.md), which refines the custody mechanism after P11 was
  measured: the service signs the claim it audits rather than one it rebuilds
