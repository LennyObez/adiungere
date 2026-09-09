# Web

> **Empty until M4.** What follows describes what will live here and why, not what is here now.

The player and export interface, written once in TypeScript and used by two surfaces: the website in
[`../apps/site`](../apps/site) and the Windows shell in [`../apps/windows`](../apps/windows). It is not an
application. It is the interface those two applications host.

It exists because a browser engine renders the first video track of a media element and nothing else. Showing
the rear camera means decoding it directly and drawing it, and showing both means doing that twice. That work
is needed for the website, so the Windows shell reuses it rather than growing a second player
([ADR-0007](../docs/adr/0007-one-desktop-answer-per-platform.md)).

Three things shape the design.

**The player is tiered, and each tier is probed rather than assumed.** Support queries lie, particularly for
encoders, so a real decode of one key frame decides which tier runs. Where the platform can play the first
track and the audio through an ordinary media element, it does; where it cannot, the low-level path takes
over; where neither works, the interface says what is missing instead of showing a black rectangle.

**Nothing leaves the device.** The container work is the Rust core compiled to WebAssembly, reading through
sliced reads so a large recording never has to be held in memory. A browser test asserts on the network that
no request leaves the origin during scan, playback or export.

**Every string comes from the catalogue.** No view composes its own sentence about integrity, because a
sentence composed per screen is a verdict waiting to be written by accident
([ADR-0005](../docs/adr/0005-the-product-never-returns-a-verdict.md)).

Populated in **M4**.
