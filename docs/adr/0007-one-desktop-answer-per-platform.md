# ADR-0007: One desktop answer per platform, not one shell for three

## Status

Accepted

## Context

Three desktop platforms, and the reflex is to pick one shell technology and use it everywhere. That reflex is
wrong here, because the three platforms do not present the same obstacles.

**macOS** shares its photo library API with iOS, and that API is the only route to a recording that lives in
the cloud rather than on disk. A separate macOS application without it would be a silently worse product on
the platform where people keep their footage in a library rather than in folders.

**Windows** has a current browser engine available in the system, which is the same engine the website
targets, and the shared player has to exist for the website anyway. A browser engine renders one video track
per element and never composites two, so a low-level decoder player is required regardless of which shell
hosts it.

**Linux** is where a hosted browser engine fails on three separate counts at once: an open upstream defect in
serving local files to the system webview, no hardware-accelerated video decoder installed by default on the
two most common distributions, and no way to reach the engine's acceleration policy from the host. The native
toolkit, by contrast, has an accessibility layer that works, a media framework with a zero-copy sink, and a
sandboxed package format that can carry its own decoders.

## Decision drivers

1. Do not ship two applications for one platform with different capabilities.
2. Do not add a second player implementation that the website does not already need.
3. Prefer one unknown layer over three.

## Decision

- **macOS** ships the multiplatform declarative target shared with iOS. No alternative macOS build is
  produced, precisely so that there is only one macOS application.
- **Windows** ships a thin shell hosting the shared player, with the core linked in process and no foreign
  function boundary. Native window materials come from the shell rather than being imitated in stylesheets.
- **Linux** ships a native toolkit shell over a two-branch media pipeline with a zero-copy sink, packaged in a
  sandbox that carries its own decoders, with the core linked directly.

Two paths are written down now rather than discovered later. The **upgrade** path for Windows is a native
media stack, conditional on two measurements. The **fallback** path for Linux is the hosted shell, conditional
on the sink probe failing.

## Alternatives considered

### One hosted shell for all three platforms

It would remove two shells and one media pipeline. It loses the system photo library on macOS, and on Linux
it fails on the three counts above, any one of which is enough on its own.

### A general-purpose media player library as the engine everywhere

It solves the dual view in a single option and is genuinely the cheapest engine available. Its licence is
reciprocal, which this project's outbound licence and dependency policy refuse.

### An immediate-mode or self-drawing user-interface toolkit in Rust

One language, no foreign function boundary, small binaries. None of them draws native controls or exposes
native accessibility, each would need a second player written against it, and the licensing of the most
complete candidate is unusable for a permissively licensed project.

## Consequences

### Positive

- Each platform gets the route that is strongest there, and no platform gets two applications.
- The Windows shell reuses the player the website needs anyway, so the second surface costs a shell rather
  than an engine.
- Linux has one unknown layer, the zero-copy sink, instead of three.

### Negative

- Three build systems, three packaging formats, three sets of platform review.
- Windows renders its controls in a document rather than with native widgets, which is a real and named
  compromise.
- The Linux shell is the only surface whose user interface is written twice, once for the web and once for the
  toolkit.

### Neutral

- The media framework used on Linux is dynamically linked from the sandbox runtime and stays outside the
  dependency graph, which needs a written licence opinion and a recorded exception.

## Enforcement

From M5, guarantee **G39**: the desktop shells never write to a source, watched with a file monitor over a
scripted session.

From M5, guarantee **G41**: the sandboxed package declares its codec extension and reports a clear
unavailable state when a user masks it.

From M5, guarantee **G40**: a stream-copy export from each shell produces fingerprints equal to the command
line's.

## What would change this decision

- The upstream local-file defect fixed and shipped in the distributions' packages, plus a measured decoder
  path: one hosted shell becomes viable again for Linux.
- Probe **P23** failing: Linux takes the hosted fallback, gated on probe **P09**.
- Probes **P08** and **P31** on Windows: a failure narrows the dual view to a default single view with instant
  switching; a success on the native stack opens the upgrade path.
- A second contributor who owns a desktop: native shells become affordable again.

## Security impact

The Windows shell hosts a document in a system browser engine, so its content security policy and its
protocol handlers are part of the threat model and are reviewed as such. The Linux shell runs in a sandbox
with a declared, minimal set of permissions.

## Privacy impact

None. No surface uploads media, and the Linux sandbox restricts file access to what a person selects through
the portal.

## Performance impact

Measured rather than assumed. Two simultaneous 1080p30 decodes plus a composite are not guaranteed anywhere,
which is why the dual view is gated by a device capability probe on every surface.

## Migration and rollback plan

Adoption is M5. Each fallback is a build target that already exists in the repository, so switching is a
packaging decision rather than a rewrite.

## Links

- [`docs/evidence.md`](../evidence.md), probes P08, P09, P22, P23, P24, P27 and P31
- [ADR-0002](0002-one-rust-core-and-thin-shells.md)
