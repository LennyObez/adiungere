# Applications

Everything that ships to a person. One directory per shipping target, each a project root in its platform's
own conventions.

| Directory | Ships as | Milestone |
|---|---|---|
| [`android`](android/README.md) | An Android application, in two flavours: one for the store and one with no proprietary services | M7 |
| [`apple`](apple/README.md) | One multiplatform target shipping to iOS, iPadOS and macOS | M8 |
| [`linux`](linux/README.md) | A sandboxed package carrying its own decoders | M5 |
| [`site`](site/README.md) | adiungere.com, an installable static application | placeholder today, M4 |
| [`windows`](windows/README.md) | A signed installer and a package manifest | M5 |

What they share lives outside this directory, because a shared implementation is not an application:
[`../core`](../core) holds everything that touches the container, [`../web`](../web) holds the player and
export interface that the website and the Windows shell both use, and [`../design`](../design) holds the
tokens every platform theme is generated from.

None of them writes a container. Every export on every surface goes through the core, including on the two
platforms whose own frameworks could do it, because those frameworks drop the vendor telemetry box
([ADR-0006](../docs/adr/0006-every-export-goes-through-the-core.md)).
