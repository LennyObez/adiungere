# Architecture decision records

A record is written for any change that touches a module boundary, a trust boundary, a dependency the product
cannot easily leave, a user-visible guarantee, or the wording the product uses about evidence.

Each record follows [`0000-template.md`](0000-template.md). Two of its sections carry most of the weight.
**Enforcement** names the test or gate that makes the decision hold, because a decision no check enforces is a
preference. **What would change this decision** names the measurement or the event that would reopen it,
because a decision with no stated reversal condition is a belief.

A record is never edited to say something different once it is accepted. It is superseded by a new one, and
its status says so.

| Record | Decision | Status |
|---|---|---|
| [ADR-0001](0001-a-single-repository.md) | A single repository for the core, the six surfaces and the website | Accepted |
| [ADR-0002](0002-one-rust-core-and-thin-shells.md) | One Rust core, thin shells, and each platform's own media engine | Accepted |
| [ADR-0003](0003-apache-2-0-with-a-notice-and-no-contributor-agreement.md) | Apache-2.0, with a notice file and no contributor agreement | Accepted |
| [ADR-0004](0004-boxes-are-opaque-byte-ranges.md) | Boxes are opaque byte ranges unless a value is needed | Accepted |
| [ADR-0005](0005-the-product-never-returns-a-verdict.md) | The product reports facts and never returns a verdict | Accepted |
| [ADR-0006](0006-every-export-goes-through-the-core.md) | Every export goes through the core, on every surface | Accepted |
| [ADR-0007](0007-one-desktop-answer-per-platform.md) | One desktop answer per platform, not one shell for three | Accepted |
| [ADR-0008](0008-the-website-runs-in-the-browser.md) | The website runs in the browser, with a service that sees only digests | Accepted |
| [ADR-0009](0009-sign-last-and-a-single-key-custody.md) | Sign last, and keep one custodian for the key | Accepted |
| [ADR-0010](0010-launch-languages.md) | English is the source language, French ships with the first product | Accepted |
| [ADR-0011](0011-documentation-is-rendered-from-the-repository.md) | Documentation lives in the repository and is rendered onto the site | Accepted |
| [ADR-0012](0012-commits-are-signed-and-carry-no-trailers.md) | Commits are signed, and carry no trailers | Accepted |
| [ADR-0013](0013-the-applications-do-not-check-for-updates.md) | The applications do not check for updates | Accepted |
| [ADR-0014](0014-the-verification-surface-is-called-check.md) | The place is called Check, the action is called verify | Accepted |
| [ADR-0015](0015-what-a-relayed-signature-proves.md) | What a relayed signature proves, and what it does not | Accepted |

## Decisions deliberately not recorded yet

Two decisions the plan expects are held back until there is something for them to constrain, because a record
whose enforcement section names no check and no milestone is a preference in a template.

- **The product name reaching every rendered surface from one value.** There is no rendered surface yet. The
  record and its guarantee arrive with M4, and the rename is exercised in M10.
- **The deployment description for the production environment.** It arrives with M4, when there is an
  application to deploy rather than a single page.
