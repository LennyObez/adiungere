# ADR-0002: One Rust core, thin shells, and each platform's own media engine

## Status

Accepted

## Context

The product does two very different things. It **manipulates a container**: reading boxes, rebuilding sample
tables, copying vendor data, computing digests, writing a manifest. And it **plays and encodes video**:
decoding two 1080p streams at once, compositing them, and encoding a result.

The first is exacting, identical on every platform, and unforgiving: a single wrong offset produces a file
that plays fine and proves nothing. The second is where the platform is better than anything that could be
bundled, because hardware decoders, colour handling, power behaviour and accessibility all come free from the
platform and cost dearly anywhere else.

Every published library that copies samples in this format starts from an empty movie box, resynthesises the
decoder configuration and drops vendor boxes. That is exactly the data this product exists to preserve.

## Decision drivers

1. The proof must be the same everywhere, which means one implementation, not six.
2. Nothing that decodes or encodes video should be bundled, on licence grounds and on size grounds.
3. A wrong byte in a container is invisible until someone challenges the file in front of a judge.

## Decision

A Rust workspace is the single implementation of everything that touches the container. It links natively
into the desktop and mobile shells and compiles to WebAssembly for the website.

Playback and composed encoding are delegated to each platform's own media engine. No media framework is
bundled or required at run time. External media tools remain development-time oracles.

Lossless remuxing, fingerprints, the manifest and its verification always go through the core, **including on
the platforms that could remux by themselves**, because their writers drop the vendor telemetry box.

## Alternatives considered

### One implementation per platform, using each platform's container API

Four implementations of the same exacting logic, four sets of golden tests, and four chances to write a
subtly different file. The platform APIs also refuse the thing the product needs most: they translate metadata
into a closed key space and lose anything they do not recognise.

### Bundle a general-purpose media library and use it everywhere

It would solve playback, composition and remuxing in one dependency. The candidates carry reciprocal licences
incompatible with this project's outbound licence and its dependency policy, add tens of megabytes to every
installer, and would still have to be taught to preserve unknown boxes.

### A cross-platform user-interface toolkit with one shell for everything

It would remove five shells. It would also remove native accessibility on every platform, remove the system
photo library on two of them, and put an unfamiliar rendering of every control in front of a person who is
about to trust the result with an insurance claim.

## Consequences

### Positive

- One place to get the format right, one corpus, one set of golden tests, one fuzzing target.
- A pipeline can compare native and WebAssembly output byte for byte on the same fixture.
- No reciprocal licence enters the dependency graph, and no encoder ships inside a stream-copy path.

### Negative

- Two foreign function boundaries to maintain, with generated bindings that must be kept fresh.
- Six playback implementations, each with its own quirks, each needing its own capability probe.
- The remuxer is the largest hand-written component in the product and has no upstream community.

### Neutral

- Each surface owns its own build and packaging, which is work that platform toolchains impose anyway.

## Enforcement

Guarantee **G11** today: library crates forbid unsafe code and refuse the panicking constructs, so the core
cannot take down a host application on a bad file.

From M2, guarantee **G25**: the stream-copy path links no encoder, proved by the dependency tree and by
inspecting the released binary for encoder symbols, with a fixture watched failing.

From M2, guarantee **G22**: the same input and the same plan produce identical bytes on Linux, macOS and
Windows, extended to WebAssembly at M4.

## What would change this decision

A platform container API that preserved unknown boxes verbatim and could be verified doing so. None does
today, and the check is cheap to repeat: probe **P02** and the export guarantees would show it immediately.

## Security impact

Positive. One parser to fuzz rather than six, written in a language that removes whole classes of parsing
defect, with unsafe code forbidden workspace-wide. The two binding crates that will need it cannot lift a
forbid, so each restates the lint table with unsafe code denied instead, justifies every site of use, and is
named in the guarantee as doing so. Until they exist, the list of such crates is empty.

## Privacy impact

Positive. Media never leaves the device on any surface, because the component that reads it is on the device.

## Performance impact

Playback and encoding run at platform speed, since they use the platform engine. Container work is bounded by
input and output rather than by processing, and the reader is required to read a header without touching the
media payload.

## Migration and rollback plan

Adoption is the initial layout. A surface can fall back to a platform API for a specific operation, but only
behind an export class that says what it produced, never silently.

## Links

- [`docs/architecture.md`](../architecture.md)
- [ADR-0004](0004-boxes-are-opaque-byte-ranges.md), [ADR-0006](0006-every-export-goes-through-the-core.md)
