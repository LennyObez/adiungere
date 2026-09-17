# ADR-0012: Commits are signed, and carry no trailers

## Status

Accepted

## Context

This repository publishes a product that asks people to trust a chain of digests and signatures. Its own
history is the first link in that chain, and the first thing a careful reader checks.

There is a related question that has to be answered at the same time, because it is a one-way door: whether
contributions carry a per-commit assertion of the right to submit them. That assertion is useful when the
outbound licence does not already settle the inbound side. Here it does, in clause 5 of the licence
([ADR-0003](0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md)).

## Decision drivers

1. A project selling verifiable provenance cannot ship an unverifiable history.
2. The inbound licence question is already answered by the outbound licence.
3. A trailer that restates what the signature already proves is noise in every log, forever.

## Decision

**Every commit is signed**, with no exception. If signing is unavailable, do not commit until it is. The
default branch's ruleset requires signatures, so an unsigned commit cannot arrive by another route.

**A milestone produces one commit** on the default branch. The pull requests that led to it remain the review
record; the commit is the delivery. History stays readable at the scale a person actually reads it.

**Commit messages carry no trailers.** The message is a Conventional Commits subject, with a scope where one
applies, and a body that states what is delivered and which check supports it. Authorship is established by
the signature, which is verifiable, rather than by a line of text, which is not. No sign-off is required,
because the licence settles what a trailer would assert.

Branch names are short and typed: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `chore`, `security`.

## Alternatives considered

### Signed commits plus a per-commit sign-off

The combination is common and costs one flag. It asserts something the licence already asserts, and it puts a
line in every message that a reader has to learn to skip.

### Merge commits, so that the shape of the work is visible

The shape of the work is visible in the pull requests, which are kept. In the history it is noise: a reader
looking for the commit that introduced a behaviour should find one commit per delivery.

### Signatures encouraged rather than required

Encouraged means present on most commits, which is the same as absent for the purpose of verifying a history.

## Consequences

### Positive

- Every commit in the history can be verified against a published key.
- The history reads as one entry per delivered milestone.
- Nothing in a commit message needs to be filtered out before reading it.

### Negative

- A contributor without signing configured cannot contribute until they set it up, which is a real barrier.
- Squashing a milestone loses the intermediate commits, so the pull request becomes the only record of how it
  was reached. That is why pull requests are never deleted.

### Neutral

- The signature format is the maintainer's; contributors may use either supported format.

## Enforcement

The default branch ruleset: signatures required, linear history, non fast-forward refused, deletion refused,
a pull request with thread resolution, squash or rebase only, named required checks, and no bypass actor. A
ruleset is enforced by the platform on every push, including the maintainer's.

A required check on every pull request refuses a title that is not a Conventional Commits subject: a type
from the list above, an optional scope, a lower-case subject with no trailing full stop, within seventy-two
characters. The title becomes the milestone commit's subject when the pull request is squashed.

One consequence is stated rather than glossed over. The commits on a branch carry the maintainer's own
signature. The squash that lands a milestone on the default branch is performed by the platform, and the
commit it writes carries the platform's signature, which the ruleset accepts and a reader can verify against
the platform's published key. The review trail from the branch's signed commits to that squash is the pull
request, which is why pull requests are never deleted.

## What would change this decision

Nothing foreseeable for signing. The one-commit-per-milestone rule would relax if the project ever had enough
parallel work that a milestone stopped being a coherent unit of delivery.

## Security impact

Positive and direct. A signed, linear, protected history is what makes it possible to state that a release was
built from a specific reviewed source, which is the foundation the release attestations build on.

## Privacy impact

The maintainer's public signing key and the commit email are published, which is inherent to the mechanism.

## Performance impact

None.

## Migration and rollback plan

Adoption is the first commit. There is no rollback: an unsigned commit cannot be added to a history that
requires signatures without rewriting it, which the ruleset also refuses.

## Links

- [`CONTRIBUTING.md`](../../CONTRIBUTING.md)
- [ADR-0003](0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md)
