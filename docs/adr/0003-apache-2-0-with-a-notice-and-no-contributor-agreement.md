# ADR-0003: Apache-2.0, with a notice file and no contributor agreement

## Status

Accepted

## Context

The product is published as open source and carries no monetisation. It is also a project whose output may be
handed to an insurer, a lawyer or a court, so the licence has to be one that an organisation's counsel
recognises without a discussion.

Two questions come together here, because answering the first without the second leaves a trap. The first is
the outbound licence. The second is what happens to code other people send, which is a one-way door: nobody
signs an agreement retroactively on behalf of a contributor who has moved on.

## Decision drivers

1. A permissive licence with an explicit patent grant, which is what makes this family safe for an
   organisation to adopt.
2. Consistency with the other open-source repositories under the same maintainer.
3. A dependency policy that can be enforced mechanically, which means an outbound licence compatible with a
   clear allow list.

## Decision

**Apache-2.0**, with a `NOTICE` file, declared in the licence file, in the workspace manifest and in the
citation metadata.

**No contributor agreement and no sign-off trailer.** Clause 5 of the licence already settles the inbound
side: a contribution intentionally submitted for inclusion is offered under the same terms unless the
contributor says otherwise. A separate agreement adds value in exactly one situation, which is keeping a
future relicence possible, and this project does not want that option enough to pay for it with an
administrative step on every outside contribution.

**No funding file.** The project takes no money, so a funding button would be a promise of a relationship
that does not exist.

## Alternatives considered

### A permissive licence without a patent grant

Shorter and just as free, and it is what most small projects reach for. It leaves an organisation's counsel
to decide what happens to patents, which is precisely the question the intended adopters ask.

### A reciprocal licence

It would keep derivatives open. It would also make the core unusable inside the closed applications this
product might one day be embedded in, and it would rule out a whole class of dependency by symmetry.

### A contributor agreement with a signing robot

It keeps a relicence and a commercial tier possible. The cost is a step that measurably reduces drive-by
contributions, paid from the first outside pull request, on a project that has none yet and whose stated
purpose is public utility rather than revenue.

## Consequences

### Positive

- Adoptable without a legal review in the organisations most likely to care about the output.
- The dependency policy follows directly: an allow list of compatible licences, permissive or copyleft
  limited to the file it covers, with anything whose terms reach the work as a whole absent from it and
  therefore refused.
- Nothing stands between a contributor and a first pull request.

### Negative

- Relicensing later would require the agreement of every contributor, which in practice means it will not
  happen. That is accepted deliberately.
- A permissive licence permits a closed derivative. That is the price of adoptability.

### Neutral

- The notice file must be carried by redistributors, which is a normal obligation of this licence family.

## Enforcement

Guarantee **G08**: the licence is declared as the same identifier in the licence file, the workspace and
every crate manifest, and the citation metadata; the licence text matches its canonical publication to the
byte; the notice file exists; and the dependency policy and the dependency review admit exactly the accepted
set of licences, with no exception. Whether the resolved graph obeys the policy is the supply-chain step of
the pipeline, run on every change and every night, and proved by adding a crate under refused terms and
watching it go red.

## What would change this decision

A decision to build a commercial tier. It would have to be taken before the first outside contribution, not
after, and it would require a contributor agreement from that day.

## Security impact

None.

## Privacy impact

None.

## Performance impact

None.

## Migration and rollback plan

Adoption is the initial state. There is no rollback once an outside contribution has been merged, which is
the reason this record exists at the first milestone rather than at the first contribution.

## Links

- [`LICENSE`](../../LICENSE), [`NOTICE`](../../NOTICE), [`CONTRIBUTING.md`](../../CONTRIBUTING.md)
- [`core/deny.toml`](../../core/deny.toml)
