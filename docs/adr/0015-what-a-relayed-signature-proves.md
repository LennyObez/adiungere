# ADR-0015: What a relayed signature proves, and what it does not

## Status

Accepted

## Context

[ADR-0009](0009-sign-last-and-a-single-key-custody.md) decided that one custodian holds the signing key: the
service, which never sees the media. It described the service as rebuilding the claim from structured facts.
Probe **P11** measured what the provenance library offers for that split, and the measurement changes the
mechanism without changing the decision.

The library separates building a manifest from signing it. The builder runs where the media is: it hashes
the asset for the hard binding, hashes each ingredient, writes the assertions, and produces the claim. The
signer is then handed one byte string, the claim's signature structure, a few kilobytes that carry the
claim itself: its generator, its title, its format, the labels of its assertions with the digest of each,
and the algorithm. The signer answers with a signature. That byte string carries no sample, no frame and
no vendor box, and this is measured: a signer that records what it is handed is given 1.7 kilobytes for a
45 kilobyte recording, and none of the recording's bytes are in it.

The same measurement showed that two builds of the same manifest are not byte-identical: the library gives
each assertion a random salt and each manifest a fresh instance identifier. A service cannot therefore
rebuild the claim from facts and compare it byte for byte with what the client built. What it can do is
decode the claim it is handed, read the labels and the digests, check them against what it accepts, and
sign that claim and no other.

## Decision drivers

1. The signature must be made where the key is, and the key must never be where the media is.
2. What the signature attests must be stated so narrowly that an adverse expert cannot show it claims more.
3. The service must be unable to sign arbitrary bytes on request.

## Decision

**The service signs a claim it has audited, not one it has rebuilt.** The client builds the manifest and
sends the signature structure of the claim, and, beside it, the assertion store the claim refers to, so
that the service can check that every digest in the claim is the digest of an assertion it was shown and
that every assertion is one this product writes. The service signs the structure it audited and returns
the signature. It signs nothing it did not decode and check.

**What a relayed signature proves.** That a claim naming these assertions by their digests was presented to
this product's signing service and signed with its key at the moment the service's time-stamping authority
attested, and that the assertions it names are the ones the service was shown. The hard binding and the
track fingerprints inside those assertions were computed on the device that had the media. The service
attests that it saw the facts; it cannot attest that the facts are true of the media, because it never had
the media.

**What it does not prove.** That the recording is what the camera wrote; the camera attests nothing and
the product says so everywhere. That the facts computed on the device were computed honestly; a modified
client could send any facts, and the service would sign them as facts it was shown. That the person who
sent the claim owns the recording. That the time is more than the authority's word, which for a
non-qualified authority carries no legal presumption.

**The wording.** Every surface that shows a signature made through the service says that it was signed by
this product's signing service from facts computed on the device. The manifest carries a signing state:
signed, pending while offline, or unsigned. Until the service exists, in M4, every signature is made
locally with a credential the process generates, and every surface says that the credential is on no trust
list.

## Alternatives considered

### Rebuild the claim on the service and compare it byte for byte

Measured impossible with the library as it is: the salts and the instance identifier are random. Fixing
them would mean patching the library's claim construction, and a claim with predictable salts loses the
property the salts exist for.

### Send the media to the service

The service would then be able to recompute every fact and its signature would attest to the media. It
would also mean footage leaving the device, which [ADR-0008](0008-the-website-runs-in-the-browser.md)
refuses and the product promises not to do.

### Sign on the device with a key in the application

Rejected in ADR-0009: a key inside a shipped application proves possession of the application.

## Consequences

### Positive

- The service's attack surface is one small, structured input that it decodes before it signs.
- The claim the product makes about a relayed signature survives a hostile reading, because it is written
  down and it is small.

### Negative

- The signature attests less than a reader may assume from the word "signed"; the wording has to carry that
  difference every time.
- The service depends on the library's claim format; a change there is a change to what the service audits.

### Neutral

- The delegated signer exists from M3 and is exercised by a test; the service that uses it arrives with M4.

## Enforcement

From M3, the provenance crate's test that a delegated signer is handed fewer than sixteen kilobytes that
carry none of the media, and that the manifest signed that way validates. From M4, guarantee **G33** on the
service's inputs, and the wording guarantee **G16** on every sentence that describes a relayed signature.

## What would change this decision

The library gaining a deterministic claim construction, which would let the service rebuild and compare.
A regulated client requiring an operator attestation over the media, which is a different tier with its
own custody model and its own record.

## Security impact

The service decodes untrusted input before it signs; the decoder is the library's, and the service refuses
anything it cannot decode or that names an assertion this product does not write. A client that lies about
its facts obtains a signature over false facts; the signature never says the facts are true, only that
they were presented.

## Privacy impact

The service sees the claim and the assertions: file names, sizes, digests, track fingerprints, the export
class and the masking choice. It sees no media, no position and no picture. What it necessarily sees, an
address and a time, is stated on the privacy page with its retention.

## Performance impact

One round trip per signing, on a payload of a few kilobytes.

## Migration and rollback plan

Nothing to migrate: no relayed signature exists before M4. If the service is withdrawn, signing falls back
to a held credential on the device that runs the command line, which is what M3 ships.

## Links

- [ADR-0009](0009-sign-last-and-a-single-key-custody.md), whose custody decision this record refines
- Probe P11 in [`docs/evidence.md`](../evidence.md)
- The delegated signer in `core/crates/adiungere-provenance/src/delegated.rs`
