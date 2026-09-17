# Contributing

## Before writing code

Read [`docs/architecture.md`](docs/architecture.md) for what owns what, and
[`docs/testing.md`](docs/testing.md) for the method. A change that touches a boundary, a data flow that
leaves the device, a dependency the product cannot easily leave, or a user-visible guarantee needs a
decision record before the code.

Two habits matter more here than in most projects, and they come from what the product is for.

**Nothing about a recording is claimed without a measurement.** If a change rests on an assumption about a
platform, a browser or a file format, that assumption is a probe with a verdict in
[`docs/evidence.md`](docs/evidence.md) before the change is merged. `unavailable` and `not started` are never
read as a pass.

**Nothing overstates what the product can prove.** The strongest true statement available is that a sequence
of bytes equals a digest recorded earlier. Everything a person actually wants to know is out of reach, and
the interface says so ([ADR-0005](docs/adr/0005-the-product-never-returns-a-verdict.md)).

## Workflow

1. Branch from `main`: `feat/`, `fix/`, `perf/`, `refactor/`, `docs/`, `test/`, `chore/`, `security/`.
2. Write the failing test first, and run it in the failing state.
3. Implement the smallest change that makes it pass.
4. Run the whole gate sequence with `scripts/gate.sh`: the whole sequence, not a scoped run. The gate needs
   the external oracles at their pinned versions; `scripts/fetch-tools.sh` places them under `.tools/`
   once, checking each digest, and the gate refuses to run without them rather than skipping the steps
   that use them.
5. Open a pull request describing what changed and what proves it.

## Commits

Conventional Commits, with a scope where the change belongs to one area:

```
feat(isobmff): keep unknown user data boxes as ranges through a remux
fix(scan): stop a paired rear file being listed as its own recording
security(relay): refuse a claim body above the cap before reading it
chore: lay the repository foundation
```

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `chore`, `security`. Scopes: the area names in
[`.github/labels.yml`](.github/labels.yml). A pull request's title follows the same form, because it becomes
the subject of the milestone commit, and a required check refuses a title that does not.

**Every commit is signed.** If signing is unavailable, do not commit until it is. The default branch requires
it, so there is no other route in.

**A milestone produces one commit.** The pull requests that led to it stay as the review record; the commit
is the delivery. **Commit messages carry no trailers**: the signature establishes authorship, and the licence
settles what a sign-off would assert ([ADR-0012](docs/adr/0012-commits-are-signed-and-carry-no-trailers.md)).

## What never enters a tracked file

Every tracked file is published. The following belong in local notes, which are untracked:

- A named past defect, or a count of what was broken.
- The development environment: a machine, a personal path, a host, a control panel, an address.
- A recording, or anything derived from one that carries a position or a face.
- Anything that reads as denigrating the product, or a comparison that names a third-party product in this
  product's category where a generic description would do.

Write what the code **guarantees** and why the property matters. The reasoning survives; the incident does
not belong in the repository. Guarantee **G05** enforces the mechanical part of this rule, and the review
checklist covers the parts a test cannot judge.

## Pull requests

Keep them small enough to review in one sitting. A change to the core and the change to every binding it
breaks belong in the same diff. That is the reason this is one repository.

The description states what changed, what proves it, and which decision record it implements or amends.

## Review checklist

- Does a test fail without this change, and was it run in the failing state?
- Is the test non-tautological? Does it assert observable behaviour rather than the steps taken?
- Does anything here silence a finding rather than fix its cause?
- Does anything here send a recording anywhere, or add a request that leaves the device?
- Does anything here open a source recording for writing?
- Does any new status read as a verdict? Is any new string outside the wording catalogue?
- Does an export path this touches still preserve the vendor telemetry byte for byte?
- Does a tracked file name a third-party product where a generic description would do?
- Does a claim in a document name a test that exists, or a milestone that will write it?

## Conduct

[`CODE_OF_CONDUCT.md`](CODE_OF_CONDUCT.md) applies here, to the maintainer on the same terms as to everyone
else.

## Reporting a security problem

Not through an issue or a pull request. See [`SECURITY.md`](SECURITY.md).

## Licensing a contribution

Clause 5 of [`LICENSE`](LICENSE) covers it: what you submit is offered under the same terms, and submitting
it confirms you hold the right to do so. There is no separate agreement to sign, and no trailer to add
([ADR-0003](docs/adr/0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md)).
