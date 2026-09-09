# ADR-0005: The product reports facts and never returns a verdict

## Status

Accepted

## Context

The output of this product may be handed to an insurer or filed in a court bundle. That makes the wording of
every status, badge and report a load-bearing part of the design rather than copy to be written at the end.

Two things constrain it. The published guidance on video authentication gives a human examiner three possible
conclusions, warns that language implying absolute certainty should be avoided, and states that metadata
cannot be relied upon in isolation. An automatic tool is not an examiner and reaches none of those
conclusions.

And the strongest true statement this product can make is narrow: that a sequence of bytes equals a digest
recorded earlier. Everything a person actually wants to know, whether the recording is genuine and whether
what it shows happened, is outside the reach of any tool.

A green tick that overstates that is not a user-interface flourish. It is the sentence an opposing expert
reads aloud.

## Decision drivers

1. A tool that overstates its evidence is worse than one that offers none, because it will be believed.
2. The person reading the screen is often not technical and is usually under stress.
3. Wording drifts. Anything left to be written per screen will be written differently on the sixth screen.

## Decision

Every user-visible statement about integrity comes from a **catalogue of controlled phrasings**, keyed by a
stable identifier, and no surface composes its own. The catalogue is the single source for translation.

The product states what it observed and where the observation came from. It never states a conclusion about
authenticity. Concretely:

- Words that assert a verdict are refused in every catalogue, in every language.
- An assertion about time always carries "no later than" and names the clock it came from.
- The integrity label of an export derives from its export class and from nothing else.
- A file with no provenance data gets a neutral state, never a negative one. Originals have no provenance
  data, which is the normal case rather than a failure.
- The ceiling on any report is a factual sentence with an explicit disclaimer that it is not an
  authentication examination.

## Alternatives considered

### Show a green tick when the digests match

It is what a person expects, and it is what every comparable interface does. It answers a question nobody
asked: matching a digest recorded by this tool proves the file has not changed **since this tool saw it**,
which says nothing about what happened before.

### Mirror the three conclusions used by human examiners

Borrowing that vocabulary would borrow authority the product has not earned. Those conclusions are the output
of an examination by a person; emitting them automatically misrepresents what was done.

### Let each surface write its own wording, with review

Six surfaces, several languages, and a reviewer who is also the author. Wording drift is not a risk here, it
is a certainty.

## Consequences

### Positive

- The product cannot be quoted as having declared something it cannot support.
- Translation has one source, so a claim cannot be softened or strengthened by a translator.
- The report is reproducible by a stranger: it names the commands that recompute every number in it.

### Negative

- The interface is less reassuring than a competing one that simply shows a tick.
- Every new state costs a catalogue entry and a review, which slows the addition of screens.

### Neutral

- The four verifier states are a design decision of this product, since the published presentation guidance
  covers assets that carry provenance data and says nothing about assets that do not.

## Enforcement

From M1, guarantee **G16**: no user-visible string in any catalogue asserts a verdict or uses internal
jargon. The check matches whole words in a verdict role, in every language directory found on disk, and the
same list is handed to the design brief.

From M1, guarantee **G17**: every time value in a manifest carries a source from a closed set, and every
sentence about time contains "no later than" and names a clock.

From M4, guarantee **G35**: the integrity label derives from the export class; a composition carries its
source fingerprints and never the sample-identical statement.

From M4, guarantee **G42**: the pixel-exact label is emitted only after every frame has been compared.

## What would change this decision

Nothing in the product. A change in the published guidance on how to present provenance to a reader would
change the presentation, never the ceiling on what is claimed.

## Security impact

None.

## Privacy impact

None.

## Performance impact

None.

## Migration and rollback plan

Adoption is the first catalogue, written in M4 with the first surface. There is no rollback: removing the
constraint would require removing the guarantees, which is a visible change to the README's guarantee table.

## Links

- [`docs/evidence.md`](../evidence.md), probes P20 and P26
- [ADR-0009](0009-sign-last-and-a-single-key-custody.md),
  [ADR-0014](0014-the-verification-surface-is-called-check.md)
