# Design

> **Empty until M4.** What follows describes what will live here and why, not what is here now.

The single source of design tokens, and the generators that turn them into a stylesheet, a mobile theme per
platform, and the desktop layers. Tokens live here as code, in the repository, and are imported into the
design tooling from here rather than the other way round, so the repository stays the source and the handoff
stays reviewable as a diff.

Three constraints are unusual enough to write down before anything is drawn.

**No colour is inherited from anywhere.** This product's identity is its own. The brand brief asks for
argued directions and a recommendation rather than imposing a palette, because choosing by inheritance is
choosing by default.

**Contrast is tested, not eyeballed, and it is tested against real frames.** The integrity states have to
stay distinguishable over a bright sky, a night scene with glare, and a wet road, which is where this
interface actually lives. A guarantee checks every text-on-surface pair and refuses a state that is
distinguishable only by colour.

**The wording is not the designer's to invent.** Every user-visible sentence about integrity comes from the
catalogue, which is also the single source for translation
([ADR-0005](../docs/adr/0005-the-product-never-returns-a-verdict.md)).

Generated artefacts are committed alongside their source and marked as generated, so they stay out of
review diffs while remaining reproducible from the tokens.

Populated in **M4**.
