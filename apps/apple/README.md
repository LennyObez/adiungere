# Apple

> **Empty until M8.** What follows describes what will live here and why, not what is here now.

One multiplatform declarative target shipping to iOS, iPadOS and macOS, over the Rust core reached through a
binary framework built by a script in [`../../scripts`](../../scripts).

macOS is here rather than in a desktop shell of its own, and that is a deliberate decision. The system photo
library is the only route to a recording that lives in the cloud rather than on disk, and it is the same
interface on both platforms. A separate macOS application without it would be a quietly worse product on the
platform where people keep footage in a library rather than in folders
([ADR-0007](../../docs/adr/0007-one-desktop-answer-per-platform.md)).

The platform renders the first enabled video track and nothing else, so every view mode is built from an
in-memory composition holding only the tracks that were asked for. That detail matters twice: a passthrough
export applied to the whole recording would quietly carry **both** cameras into a file someone asked to
contain one.

Exports go through the core, not through the platform writer, because the platform translates metadata into
a closed key space and the vendor telemetry does not survive the trip.

Publishing depends on a developer programme membership, which is named as a prerequisite of the milestone
rather than assumed. Building and testing run on free runners without it.

Populated in **M8**.
