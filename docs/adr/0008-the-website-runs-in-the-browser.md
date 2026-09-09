# ADR-0008: The website runs in the browser, with a service that sees only digests

## Status

Accepted

## Context

The website has to do what the applications do: find dashcam clips, play either camera, export losslessly,
and produce a manifest. The obvious architecture uploads the file and does the work on a server, where a
single implementation is easy to operate and nothing depends on what a browser supports.

The recordings in question are evidence. They contain a route, satellite positions, timestamps, and often
other people's vehicles and faces. Uploading them to anyone's server, including this project's, changes what
the product is: it becomes a party that has held a copy of the evidence and can be asked what it did with it.

Two things nonetheless cannot happen in a browser. A page cannot call a time-stamping authority, because those
services do not answer cross-origin requests. And a signing key cannot live in a page, or in a downloadable
binary, because either one is extractable by anyone who wants it.

## Decision drivers

1. Media must never leave the device. That is the product's strongest claim and the reason it can be used for
   a claim file at all.
2. Something must nonetheless reach the network for time stamping and signing to be possible.
3. Whatever runs on a server must be small enough to describe in a paragraph and audit in an afternoon.

## Decision

The website is a **static application**. Reading, playing, exporting and checking all happen in the browser,
through the same core compiled to WebAssembly.

A **minimal service** sits behind it and sees only digests and structured facts, never media. It exposes two
routes: one that relays a time-stamp request for a 32-byte digest, and one that rebuilds a provenance claim
from structured facts and signs it. Bodies are capped, there is no upload route of any kind, and no request
body is logged.

The service is also the **single custodian of the signing key**, for every surface including the native ones.
A key shipped inside a downloadable application is a key anyone can extract, and a manifest signed with an
extracted key would be worse than an unsigned one.

The check page is entirely client-side. A reader drops a file and a manifest, the core recomputes, and the two
are compared without an account and without an upload.

## Alternatives considered

### Server-side processing

One implementation, no browser capability matrix, no memory limits. It makes this project a holder of other
people's evidence, with the retention questions, the disclosure exposure and the trust burden that follow. It
also removes the sentence that makes the product credible.

### Fully offline, with no service at all

Nothing to operate, nothing to trust. It also means no time stamps, since a browser cannot reach an authority
directly, and no signature that any external verifier recognises. The product would still produce manifests
and fingerprints, which is the fallback if the signing flow proves impossible, but it is a weaker product.

### A key inside each application, so that signing happens on the device

It sounds stronger, because the signature is made where the file is. The key is extractable from every copy
of the application, so the signature would attest to nothing, and publishing it would put a worthless
credential onto a trust list.

## Consequences

### Positive

- Media never leaves the device, on every surface, and the claim is checkable with a network assertion in the
  browser test suite.
- The service holds no media, so there is nothing to retain, nothing to disclose and nothing to breach.
- The check page works for a stranger with no relationship to this project.

### Negative

- Browser capability varies by engine and by device, so the player needs tiers and every tier needs a probe.
- Large exports run into browser memory and download limits that a server would not have.
- The signature attests that a set of facts was **presented** to the service, not that the service observed
  the media. That distinction has to be stated in the interface rather than glossed over.

### Neutral

- The service is a small binary behind the site's own host, and can be replaced by an equivalent that answers
  the same two routes.

## Enforcement

From M4, guarantee **G34**: no request leaves the origin during scan, playback or export, apart from the
digest calls to the service, recorded by the browser test suite on three engines.

From M4, guarantee **G33**: the service refuses any body over the cap, accepts no multipart field, and logs no
body.

From M4, guarantee **G32**: the bundle constructs no shared memory and uses no atomics, so the site needs no
cross-origin isolation headers.

## What would change this decision

A client with a regulatory requirement for a server-side chain of custody with an operator attestation. That
is a separate product tier with its own consent, its own retention and a record that replaces this one. It is
never the default.

## Security impact

The service is the only network-reachable component and the only key custodian, so it is the whole
server-side attack surface: two routes, capped bodies, rate limiting, and claims rebuilt from structured
facts rather than signed as opaque input.

## Privacy impact

The strongest possible: no media is transmitted. What any server necessarily sees, an address and a time, is
stated on the privacy page along with how long it is kept.

## Performance impact

Export speed is bounded by the browser's encoder and by memory. The WebAssembly writer must stream from
sliced reads within a bounded budget, which is probe **P10**, and the alternative is an external fragmenter.

## Migration and rollback plan

Adoption is M4. If the split signing flow proves impossible, the site still produces manifests and time
stamps and says plainly that exports carry no Content Credentials.

## Links

- [`docs/evidence.md`](../evidence.md), probes P06, P07, P08, P10, P11, P12, P19, P25, P28 and P29
- [ADR-0009](0009-sign-last-and-a-single-key-custody.md)
