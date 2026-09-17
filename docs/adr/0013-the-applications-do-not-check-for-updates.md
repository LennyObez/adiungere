# ADR-0013: The applications do not check for updates

## Status

Accepted

## Context

An installed application usually asks a server whether a newer version exists. It is expected, it is easy, and
almost every desktop product does it.

This product's central claim is that nothing leaves the device. A background request on every launch is
something leaving the device: an address, a time, a version and an implicit installation count, sent by an
application a person opened to look at footage of a collision. It is a small leak, and it is the exact shape
of the thing the product tells people it does not do.

Every channel this product ships through already updates applications: the two mobile stores, the Windows
package manager, and the Linux sandbox repository each pull updates without the application asking anything.

## Decision drivers

1. A claim that holds everywhere except in one background request does not hold.
2. The distribution channels already solve the problem, on every platform the product ships to.
3. An update mechanism inside the application is also an update mechanism an attacker would like to reach.

## Decision

**No application checks for updates, on any surface.** There is no version endpoint, no update service and no
background request of any kind.

The about panel shows the version, the commit it was built from, and the command that lists published
releases, so a person who wants to know can find out in one step and by their own action.

The website is different by nature: a page is fetched when it is opened, and its service worker updates the
cached application when the person loads it. That is the browser's own mechanism, not a check this product
performs.

## Alternatives considered

### Check on launch, with a setting to turn it off

A setting that is on by default is the behaviour, and a setting that is off by default is a feature nobody
finds. Either way the code path exists and can be reached.

### Check only when the person opens the about panel

Better, and still a request the product would have to disclose, for information the store already delivers.
The command in the about panel gives the same answer without the product acting on its own.

### Bundle a self-updater with signed packages

It is what an application distributed outside a store would need. Every channel here is a store or a package
manager, so it would duplicate an existing mechanism and add a privileged writer to the installation
directory.

## Consequences

### Positive

- The privacy claim is complete rather than nearly complete, and the browser test that asserts no request
  leaves the origin has a counterpart on every installed surface.
- No update code path exists to attack, and no server has to stay available for the applications to behave
  correctly.
- Nothing is collected, so nothing has to be disclosed or retained.

### Negative

- A person who installs outside a managed channel will not be told that a newer version exists.
- The project loses the usage signal that an update check incidentally provides. That signal was never going
  to be collected anyway.

### Neutral

- Release notes and the release list remain public and linkable.

## Enforcement

Guarantee **G53**, from M5 and extended to each later surface: an installed application is watched over a
scripted session and must make no network request during scan, playback, export or verification, apart from
the signing and time-stamping calls that a person explicitly triggers. That is the installed counterpart of
guarantee **G34**.

## What would change this decision

Distribution outside every managed channel, for example a direct download that a significant number of people
use. That would need a signed update mechanism, and it would need this record replaced rather than amended.

## Security impact

Positive. One fewer network client, one fewer server dependency, and no privileged writer to the installation
directory.

## Privacy impact

Positive and central. No installation is counted, no address is collected, and no launch is observed.

## Performance impact

Positive, marginally: no request on launch.

## Migration and rollback plan

Adoption is the first shipped application. Rollback would mean adding a network client, which is a visible
change to the guarantee above.

## Links

- [ADR-0008](0008-the-website-runs-in-the-browser.md)
- [`docs/roadmap.md`](../roadmap.md), M5 and M7
