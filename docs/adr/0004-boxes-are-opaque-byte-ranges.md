# ADR-0004: Boxes are opaque byte ranges unless a value is needed

## Status

Accepted

## Context

The recordings this product exists to handle carry a vendor box inside the user-data box, roughly thirty
kilobytes of it on the reference clip, holding the camera identity, one satellite fix per second and about
twenty motion records per second. There is a second vendor box at the top level, four bytes of payload. Both
are undocumented. Neither appears in any public registry, in any metadata tool, or in any library.

Every route through a general-purpose library loses them. Typed box libraries parse the containers they know
and **discard the children they do not**, which is not a defect but the consequence of modelling a format as
types. Platform media frameworks translate metadata into a closed key space with no room for an unknown box.
The common command-line tool has no path for passing an unrecognised user-data box through to a muxer.

The user's own extraction, made with that tool before this project began, is the demonstration: correct video,
correct audio, and the telemetry and the camera identity gone.

## Decision drivers

1. What must be preserved is precisely what no library recognises.
2. Preservation must not depend on interpretation. A parser that has to understand a box before it can copy it
   will lose the box on the first firmware it has never seen.
3. A wrong offset is invisible until someone challenges the file.

## Decision

The box tree is a tree of **byte ranges**: an offset and a length into the source, with a type. A range is
copied verbatim unless a value inside it is actually needed.

Typed views exist only for the boxes whose values the product must read or rewrite: the file type, the movie
and track headers, the media header and handler, the sample tables, the sync sample table, the composition
offsets and the edit list.

Three things are **never** modelled as types:

- The user-data box and every child of it.
- Unknown top-level boxes.
- Sample description entries and their children, including the decoder configuration record. These are copied
  as ranges, never decoded and resynthesised, because a typed container would drop an unknown child inside
  them exactly as it does anywhere else.

There is no typed user-data structure anywhere in the codebase, and no code path constructs one.

## Alternatives considered

### Use a typed box library and add the vendor boxes to it

It would give a readable model and upstream maintenance. It requires knowing every box in advance, which is
the assumption this format breaks: the next firmware, or the next manufacturer, writes something nobody has
modelled, and the failure is silent.

### Parse the vendor box and rewrite it faithfully

Faithful rewriting requires understanding the layout completely, including whether any field inside it is an
absolute file offset. Probe **P02** exists to answer that question, and until it does, any rewrite is a guess
applied to the evidence the product is meant to protect.

### Copy the whole movie box unchanged

That would preserve everything and produce a file whose sample tables describe media that is no longer there.
Extraction necessarily rebuilds the tables; the point of this decision is that it rebuilds **only** them.

## Consequences

### Positive

- Vendor telemetry and unknown boxes survive every export, whether or not anyone has decoded them.
- The reader stays small and the parsing attack surface stays narrow, because most boxes are never parsed.
- Reading a header never touches the media payload, so inspecting a clip in a cloud library is cheap.

### Negative

- The remuxer is written by hand rather than taken from a library, and it is the largest hand-written
  component in the product.
- Ranges are less pleasant to work with than types, and errors in offset arithmetic are the failure mode to
  fear.

### Neutral

- The telemetry parser is a separate, read-only, best-effort concern that degrades to "bytes preserved" and
  can never block an export.

## Enforcement

From M1, guarantee **G12**: parsing then reserialising without an edit reproduces the input byte for byte,
including an unknown child inside a sample description entry, with a fixture built for that case.

From M1, guarantee **G13**: no code path constructs a typed user-data structure, and an unknown child of the
user-data box survives every public entry point.

From M2, guarantee **G21**: every lossless export preserves the vendor box and the unknown top-level box byte
for byte, with a deliberately broken branch watched failing.

## What would change this decision

Probe **P02** finding absolute file offsets inside the vendor box. That would not make typing it right; it
would mean verbatim copying has to be accompanied by a manifest field and a sentence saying that internal
offsets, if any, were not adjusted. The rewrite stays refused either way.

## Security impact

Positive. Bytes that are never parsed cannot be parsed wrongly. The boxes that are parsed are fuzzed from M1.

## Privacy impact

The preserved vendor bytes contain satellite positions. That is a reason to preserve them faithfully in an
export the owner chooses to make, and a reason the reference recording is never published and the masking
step precedes every share affordance.

## Performance impact

Positive. Copying a range is a read and a write with no interpretation, and the reader can work in a bounded
window rather than holding a file in memory.

## Migration and rollback plan

Adoption is the initial design of the reader. There is no rollback: a typed model would have to prove
guarantee **G12** first, and it cannot.

## Links

- [`docs/evidence.md`](../evidence.md), probes P02 and P15
- [ADR-0006](0006-every-export-goes-through-the-core.md)
