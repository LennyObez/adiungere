# Windows

> **Empty until M5.** What follows describes what will live here and why, not what is here now.

A thin shell hosting the shared player from [`../../web`](../../web), with the core linked in process and no
foreign function boundary.

The reasoning is short. A browser engine renders one video track per element and never composites two, so a
low-level decoder player has to exist for the website regardless. The engine available in the system is a
current one where that path is established. Hosting it costs a shell rather than a second player, and the
window materials come from the shell rather than being imitated in stylesheets
([ADR-0007](../../docs/adr/0007-one-desktop-answer-per-platform.md)).

The named compromise: controls are rendered in a document rather than as native widgets. The upgrade path is
written down rather than left as a regret. A native media stack replaces the hosted player if two
measurements say it should, and both are probes with fallbacks already chosen.

One measurement gates the shell itself: whether demultiplexed packets can be fed to the hosted page fast
enough to seek in under a tenth of a second, or whether a file handle should be passed instead.

Publishing a signed installer depends on a code-signing arrangement, which is a prerequisite of the
milestone rather than an assumption. A public release is never unsigned for a product that sells integrity;
internal test builds may be.

Populated in **M5**.
