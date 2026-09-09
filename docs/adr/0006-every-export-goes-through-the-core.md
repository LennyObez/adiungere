# ADR-0006: Every export goes through the core, on every surface

## Status

Accepted

## Context

Two of the six surfaces can already write a container by themselves. The Apple frameworks can pass a track
through without re-encoding it, and the Android transformer can transmux one. Using them would remove a large
amount of work on exactly the two platforms where the work is hardest.

Both of them lose the vendor telemetry box on the way. One translates metadata into a closed key space; the
other recognises four kinds of user-data child and writes five. Neither is a defect in those frameworks. It is
what a general-purpose media pipeline does with data it does not model.

There is a subtler trap in the same place. A passthrough export on Apple applies to **every** track present,
so exporting "just the rear camera" from the whole asset quietly produces a file with both cameras in it,
unless the export is built from a composition containing only the track that was asked for.

## Decision drivers

1. The telemetry is the reason a claims handler will believe the file, and it is what every platform writer
   discards.
2. Output must be byte-identical across surfaces, which cannot happen if two surfaces use different writers.
3. Fingerprints must be computed on the bytes that were actually written, by the component that wrote them.

## Decision

Every export on every surface is written by the core: the lossless extraction, the two-track archive, and the
finalising step that wraps a composition produced by a platform encoder.

Platform media frameworks are confined to two jobs: **playing** frames on a screen, and **encoding** a
composed rendition. A composed rendition is handed back to the core as an elementary stream, and the core
writes the container, copies the vendor bytes, computes the fingerprints and emits the manifest.

No surface ever opens a source file for writing.

## Alternatives considered

### Use each platform's writer and reattach the metadata afterwards

Reattaching requires rewriting a container that has already been written, which changes offsets, invalidates
any signature applied to it, and reintroduces the class of bug this decision avoids. It also produces a
different file on each platform.

### Use the platform writer only for the composed rendition, where nothing is lossless anyway

Tempting, because a re-encoded file has already lost the pixel-level argument. It would still lose the
telemetry, and a composition is precisely where the source fingerprints and the vendor bytes matter most,
since the pixels can no longer speak for themselves.

## Consequences

### Positive

- One writer, one set of golden tests, one fuzzing target, one answer to "which bytes did you write".
- The same input and plan produce the same digest on a phone, a laptop and a browser tab.
- A composition can carry its sources' fingerprints and the original vendor bytes, which is what makes it
  usable next to the originals rather than instead of them.

### Negative

- The mobile surfaces cannot take the short path their frameworks offer.
- The foreign function boundary carries file handles and progress rather than being avoided entirely.

### Neutral

- Platform frameworks remain first-class for playback, which is where they are unbeatable.

## Enforcement

From M2, guarantee **G24**: no surface ever opens a source for writing, proved by a reader type with no write
capability and by digesting and timestamping sources before and after a scripted session on each shell.

From M5 onwards, guarantee **G40**: a stream-copy export performed by each shell produces fingerprints equal
to those the command line produces for the same plan.

From M2, guarantee **G21**: every lossless export preserves the vendor box byte for byte.

## What would change this decision

A platform writer that could be shown to preserve an arbitrary unknown user-data child byte for byte. The test
already exists in the form of guarantee **G21**, so the claim would be cheap to check and is checked whenever
a platform framework changes.

## Security impact

Positive. One writer to review and fuzz. Sources are opened read-only on every surface, so a defect in an
export path cannot damage the recording it was asked to protect.

## Privacy impact

None beyond the product's general rule that media never leaves the device.

## Performance impact

Neutral to positive for extraction, which becomes a bounded copy rather than a framework pipeline. Composition
is unchanged, since the encoding still happens on the platform encoder.

## Migration and rollback plan

Adoption arrives with M2 for extraction and M4 for composition. A surface that cannot yet call the core does
not ship an export button; it does not ship a platform export instead.

## Links

- [ADR-0002](0002-one-rust-core-and-thin-shells.md), [ADR-0004](0004-boxes-are-opaque-byte-ranges.md)
- [`docs/evidence.md`](../evidence.md), probe P01
