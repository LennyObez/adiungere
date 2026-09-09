# ADR-0011: Documentation lives in the repository and is rendered onto the site

## Status

Accepted

## Context

Two documents in this project have to be readable by people who will never clone a repository: the integrity
specification, which tells a stranger how to reproduce every number in a manifest without this product, and
the verification guide, which is written for a driver, a claims handler and a lawyer.

They also have to be reviewed as code, because they state what the product guarantees, and a claim in a
document that no test supports is exactly the failure this project is built to avoid.

## Decision drivers

1. A specification that is not in the repository drifts from the code that implements it.
2. A specification a lawyer cannot open in a browser is a specification a lawyer will not read.
3. The documentation site must not need a second toolchain, a second deployment or a second hosting story.

## Decision

**The repository is the source.** Every document lives in `docs/` as Markdown, is reviewed in the same pull
request as the code it describes, and is checked by the same pipeline.

**The site renders it.** The documentation section of the website is generated from those files by the
website's own static build and published under `/docs` on the same domain, with no separate deployment.

Documentation examples that can be executed are executed: Rust documentation tests are part of the gate
sequence, so an example that stops compiling fails the build rather than misleading a reader.

## Alternatives considered

### A separate documentation site with its own toolchain and its own hosting

More capable, and immediately a second thing to deploy, secure, update and keep in step with the product. On
a one-maintainer project it becomes the part that is out of date.

### Rely on the repository's own rendering of Markdown

Free, and already correct today. It is fine for contributors and wrong for the audience that matters here: a
claims handler will not be sent to a source-hosting site to read how to check a file.

### Documentation generated from source comments only

Excellent for an interface reference and useless for a specification, a guide, or a decision record.

## Consequences

### Positive

- One source, one review, one pipeline, one deployment.
- A relative link that breaks fails the pipeline before anyone reads it.
- The specification and the code that implements it move in the same commit.

### Negative

- The site build gains a step, and a document with a rendering-specific feature has to be written in plain
  Markdown that both surfaces render the same way.

### Neutral

- The rendered output is generated and is not tracked, so the repository stays the only source.

## Enforcement

Guarantee **G04** today: every relative link in a tracked Markdown file resolves, and the decision index lists
every record. Both are watched failing before being trusted.

Guarantee **G09** today: prose wraps at 110 columns, so a document is reviewable as a diff rather than as one
long line per paragraph.

From M4, the site build fails when a document referenced by the navigation does not exist.

## What would change this decision

A volume of documentation large enough to need search, versioning across releases and an interface reference
in one place. The move would be a change of renderer, not a change of source, because the source is already
the repository.

## Security impact

None. The rendered output is static.

## Privacy impact

None. The documentation section carries no analytics and no third-party requests, like the rest of the site.

## Performance impact

None meaningful. Static pages, long cache lifetimes on immutable assets.

## Migration and rollback plan

Adoption is M4 with the site. Until then the documents are read in the repository, which is where they live
anyway.

## Links

- [`docs/testing.md`](../testing.md), the gate sequence
- [ADR-0008](0008-the-website-runs-in-the-browser.md)
