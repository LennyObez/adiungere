# Linux

> **Empty until M5.** What follows describes what will live here and why, not what is here now.

A native toolkit shell in Rust over a two-branch media pipeline, packaged in a sandbox that carries its own
decoders. The core links directly, with no foreign function boundary.

This is the one surface where hosting a browser engine was rejected on findings rather than on taste, and
the findings are probes that have to hold before a line of shell code is written.
Serving local files to the system webview is broken upstream, hardware video decoding is not installed by
default on the two most common distributions, and the engine's acceleration policy cannot be reached from a
host application. The native toolkit answers all three: its accessibility layer works, its media framework
has a zero-copy sink, and the sandbox can ship the decoders the host does not have
([ADR-0007](../../docs/adr/0007-one-desktop-answer-per-platform.md)).

Two probes run **before** any shell code is written, because both can invalidate the choice: whether the
zero-copy sink really gives a two-branch pipeline across display protocols and graphics vendors inside the
sandbox, and whether the runtime codec extension decodes this profile on stock distributions. If the first
fails, the fallback is the hosted shell, gated on its own probe. If a person masks the extension, the
application says so clearly rather than showing a black rectangle.

The media framework is dynamically linked from the sandbox runtime and stays outside the dependency graph.
That needs a written licence opinion and a recorded exception, both prerequisites of the milestone.

The sandboxed package is the only Linux channel at M5. A self-contained single-file build is deferred,
because bundling the media framework and a decoder without the packaging tool that usually does it is not
studied anywhere, and shipping one on a guess is not a channel.

Populated in **M5**.
