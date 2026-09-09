# Infrastructure

> **Empty until M4.** What follows describes what will live here and why, not what is here now.

How an environment is described, and how a change reaches it. Today there is one static page and no
application to deploy, so there is nothing here that a description would describe.

Two rules apply from the first deployment, and both are already in force for the placeholder page.

**What is served is a checkout of a signed commit.** The deployment pulls the default branch and publishes
one directory of it, so what a visitor receives can be traced back to a commit whose signature verifies.
Nothing is built on the server, and no artefact is uploaded by hand.

**Nothing here describes the environment it runs in.** No provider, no control panel, no host name, no
address, no personal path. Every tracked file is published, and a document that names the operating stack of
a running service is a hint written for whoever goes looking. Guarantee **G05** enforces the mechanical part
of that; the rest is a review question in [`../CONTRIBUTING.md`](../CONTRIBUTING.md). Operating notes and
credentials live outside the repository entirely.

What will land here in **M4**: the response header policy the site needs, the media types it depends on, the
cache lifetimes, and the operating runbook for the signing and time-stamping service, including key rotation
and revocation. The service is the only network-reachable component and the only key custodian
([ADR-0009](../docs/adr/0009-sign-last-and-a-single-key-custody.md)), which is why its runbook is a
deliverable rather than a note.

Populated in **M4**.
