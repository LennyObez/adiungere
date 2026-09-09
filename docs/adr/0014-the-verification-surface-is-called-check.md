# ADR-0014: The place is called Check, the action is called verify

## Status

Accepted

## Context

The product has a surface where a stranger drops a file and a manifest and finds out what can be said about
them. Naming it is not decoration: the name is the first claim the product makes, and it is read before
anything on the page.

The obvious name collides with the rule that governs the whole product. Words in the verified family are
refused in every catalogue, because they assert a conclusion the product cannot reach
([ADR-0005](0005-the-product-never-returns-a-verdict.md)). A page called by that name would announce a
verdict in its own title, above a body that carefully avoids giving one.

The verb is a different question. Across the provenance and supply-chain tools that this product's users
already run, the action is universally called verify. Renaming the action would make the command line
unfamiliar to exactly the people most likely to use it, and it would gain nothing, because a verb naming an
action is not a claim about a file.

## Decision drivers

1. A place name is read as a claim; a verb is read as an instruction.
2. Familiarity in the command line has real value for the technical reader who will check the work.
3. The distinction has to be written down, or the next person will unify the two and reintroduce the verdict.

## Decision

The **surface** is called **Check**, on the website at `/check` and in every application. It is what a person
navigates to.

The **action** keeps the conventional verb: `adiungere verify`. It is what a person performs.

The **result** never uses either word as a status. It states what was observed, with its source, in the
controlled wording.

## Alternatives considered

### Call the surface by the conventional name too

Consistent with the command line and with what people search for. It puts a verdict in the page title, which
is the one thing the product must not do.

### Rename the action to match the surface

Internally consistent, and it makes the command line unfamiliar to the audience most likely to check the
project's own claims.

### Call the surface something neutral and abstract

It avoids the problem by saying nothing, and leaves a stranger unsure whether they are in the right place.

## Consequences

### Positive

- The strongest claim on the page is a neutral one, and the body can say exactly what was found.
- The command line matches what the technical audience already types.
- The distinction is documented, so unifying the two names now requires a decision rather than a preference.

### Negative

- Two names for one concept, which has to be explained once in the documentation and in the interface.
- People searching for the conventional word have to find the page another way, which the page copy handles.

### Neutral

- Translations follow the same split: a neutral noun for the place, the conventional verb for the action, both
  from the catalogue.

## Enforcement

From M1, guarantee **G16**: catalogue strings are matched as whole words in a verdict role, so the verb naming
an action is permitted and a status asserting a conclusion is not, in every language.

From M4, the site route is `/check` and the navigation label comes from the catalogue like every other string.

## What would change this decision

Nothing internal. If the wider ecosystem settled on a different verb for the action, the command line would
follow the ecosystem, and the surface would keep its neutral name for the same reason it has one now.

## Security impact

None.

## Privacy impact

None. The surface is client-side and uploads nothing.

## Performance impact

None.

## Migration and rollback plan

Adoption is M4 with the first surface, and the command line verb arrives in M1. There is nothing to migrate.

## Links

- [ADR-0005](0005-the-product-never-returns-a-verdict.md)
- [`docs/evidence.md`](../evidence.md), probe P20
