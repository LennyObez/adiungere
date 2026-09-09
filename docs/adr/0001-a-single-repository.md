# ADR-0001: A single repository for the core, the six surfaces and the website

## Status

Accepted

## Context

The product is one implementation of a container format serving six surfaces: iOS, macOS, Android, Windows,
Linux and a website. Every surface calls the same core through a different binding, and every surface must
produce byte-identical output for the same input, because the whole value of the product is that a file
exported on a phone and a file exported on a laptop carry the same digest.

Splitting a polyglot product across repositories is the common reflex. It is also how a core silently drifts
from its consumers: the core changes a field, the bindings follow days later, and nothing in either repository
fails at the moment the mistake is made. For a product whose output is used as evidence, a version skew
between two surfaces is not an inconvenience; it is two answers to the same question.

## Decision drivers

1. A change to the core and the change to every binding it breaks must land in one diff.
2. Six surfaces multiply the ways a directory can be added that no pipeline builds.
3. One maintainer. Coordination across repositories is a cost paid every day for a benefit taken rarely.

## Decision

One repository holds the Rust core, the shared player, the six surface projects, the website, the design
tokens and the documentation. Each surface is a top-level directory with its own pipeline and its own path
filter.

## Alternatives considered

### One repository per surface, with the core published as a package

Every surface would consume a released core, which sounds cleaner and defers integration until a release.
That deferral is exactly the problem: the moment a breaking change is made is the moment it should be seen,
and a published package moves that moment weeks later. It also multiplies release ceremony by seven for a
project with one maintainer.

### Core in one repository, all surfaces in a second

The split would fall on the one boundary that matters most and is crossed most often. It buys a smaller
checkout and pays with the same drift, at the same cost.

## Consequences

### Positive

- A contract change updates the core, the generated bindings and every affected surface in one reviewable
  diff.
- One issue tracker, one milestone set, one project board, one release ceremony per artefact.
- A stranger reads the whole product without discovering which of seven repositories holds the answer.

### Negative

- The checkout carries platform projects a given contributor will never build.
- Path filters become load-bearing: a filter that is too narrow silently skips a build.
- Release tags must be prefixed per artefact, because one repository publishes several things.

### Neutral

- Pipelines are per artefact rather than per repository, which is the same work arranged differently.

## Enforcement

Guarantee **G03**: every directory holding non-Markdown source is covered by a workflow path filter, with the
directories discovered from `git ls-files` rather than from a list written by hand. A hard-coded list cannot
detect the very thing the check exists to catch, which is a directory nobody remembered.

Guarantee **G10**: a directory that is empty until a milestone says so in its own README, and a directory that
holds source is not marked empty.

## What would change this decision

A second contributor or team taking ownership of one surface, with its own release cadence and its own
reviewers. At that point the coordination cost this decision avoids is being paid anyway, and a split becomes
cheaper than the alternative.

## Security impact

None directly. One consequence is worth naming: a single repository means one set of pipeline tokens, so the
default of read-only permissions and per-job escalation matters more here than it would across seven
repositories.

## Privacy impact

None.

## Performance impact

None on the product. Continuous integration cost is controlled by path filters on pushes, with every workflow
still running on pull requests, because a filtered required check never fires and therefore never merges.

## Migration and rollback plan

Adoption is the initial layout. Reverting means extracting a surface directory with its history and
republishing the core as a package, which stays possible because the core is already a workspace with no
dependency on any surface.

## Links

- [`docs/architecture.md`](../architecture.md)
- [`docs/roadmap.md`](../roadmap.md)
