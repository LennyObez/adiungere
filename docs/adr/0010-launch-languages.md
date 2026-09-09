# ADR-0010: English is the source language, French ships with the first product

## Status

Accepted

## Context

Every user-visible statement about integrity comes from a controlled catalogue, and that catalogue is the
single source for translation ([ADR-0005](0005-the-product-never-returns-a-verdict.md)). That changes what a
translation is in this product. Elsewhere a loose translation is a rough edge. Here it can promote an
observation into a verdict, or drop the qualification that keeps a sentence true, in a document someone hands
to an insurer.

The first jurisdictions whose treatment of this kind of recording was researched are French-speaking and
Dutch-speaking. The maintainer reads both of those and German.

## Decision drivers

1. A wording rule that only holds in one language is not a wording rule.
2. Everything that reaches the disk or a platform is English, as in every public repository under this
   maintainer.
3. A language nobody can review is a liability rather than a feature.

## Decision

**English is the source language** for the product and for the repository: identifiers, file and directory
names, enumeration values, keys, documentation, commit messages, code and comments.

**French ships with the first product**, at M4, as a fully reviewed catalogue. **Dutch and German follow**
when their catalogues can be reviewed with the same care.

A language is added only with a reviewed catalogue. There is no machine-translated fallback for an integrity
statement, in any surface, ever.

Catalogues are **discovered from the directory**, never from a list written in a test or a build file, so the
forbidden-verdict guarantee automatically covers a language on the day someone adds it.

## Alternatives considered

### English only until much later

Simplest, and it delays the whole question. It also puts an English-only integrity report in front of the
French-speaking claims handlers who are the first realistic readers.

### Many languages at launch, machine-translated and corrected later

It is the usual way to reach more people quickly. In this product it means shipping sentences that assert
things nobody checked, in the exact place where overstating is the worst thing the product can do.

### A list of supported languages in the build

It reads as more controlled. It is the opposite: the day someone adds a catalogue and forgets the list, the
guarantee stops covering it and nothing says so.

## Consequences

### Positive

- The wording rule is enforced in every language present, without anyone remembering to extend it.
- Adding a language is adding a reviewed catalogue, with no other step.
- One source language keeps identifiers and documentation consistent for contributors.

### Negative

- Fewer languages at launch than a machine-translated product would claim.
- Each new language costs a review by someone who reads it, which is a real constraint on a one-maintainer
  project.

### Neutral

- The working notes of the project are kept in the maintainer's language and are untracked, which is a
  separate matter from the product.

## Enforcement

From M1, guarantee **G16**: no user-visible string in **any** catalogue found on disk asserts a verdict or
uses internal jargon, matched as whole words in a verdict role, per language.

From M1, guarantee **G17**: every sentence about time contains the "no later than" construction and names its
clock, in every language.

## What would change this decision

A contributor who reads a language and will review its catalogue. That is exactly what unlocks a new
language, and it is the only thing that does.

## Security impact

None.

## Privacy impact

None. Language is chosen from the platform's own setting; nothing is sent anywhere to determine it.

## Performance impact

None. Catalogues are static data loaded per language.

## Migration and rollback plan

Adoption is M4 with the first catalogues. Removing a language means removing its catalogue directory, after
which the guarantee stops scanning it automatically.

## Links

- [ADR-0005](0005-the-product-never-returns-a-verdict.md)
- [`docs/roadmap.md`](../roadmap.md), M4
