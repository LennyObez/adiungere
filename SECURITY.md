# Security policy

## Reporting a vulnerability

Report privately through the repository's private vulnerability reporting: **Security, then Report a
vulnerability**. That channel is monitored and keeps the report confidential until a fix is available.

Please do not open a public issue for a security problem, and please do not disclose it publicly before a
fix has shipped.

A useful report contains what an engineer needs to reproduce the problem: the affected component, the input
or request that triggers it, what you observed, and what you expected. A proof of concept helps; a working
exploit is not required and is not expected.

**Please do not attach a recording.** If a malformed file triggers the problem, describe its structure, or
send the smallest synthetic file that reproduces it. The repository holds a fixture generator for exactly
this purpose from M1.

The machine-readable contact is published at `/.well-known/security.txt` on the project's domain.

## What to expect

| Stage | Timing |
|---|---|
| Acknowledgement that the report was received | Within 3 working days |
| Initial assessment, with a severity and a direction | Within 10 working days |
| Fix for a critical or high-severity issue | Prioritised over feature work |
| Coordinated disclosure | Agreed with the reporter before publication |

Reporters who wish to be credited are credited in the release notes.

## Scope

In scope: the Rust core, the command line, the shared player, the website, the signing and time-stamping
service, the platform applications, and the release pipeline that builds and attests them.

Out of scope: findings that require physical access to an unlocked device; reports produced by an automated
scanner with no demonstrated impact; denial of service through volume alone; and vulnerabilities in
third-party dependencies that already have a public advisory, which the dependency update process handles.

## What this product protects, and how

The threat model of a tool used for evidence is not the usual one. Three properties matter more than the
rest, and each is named here with the milestone that enforces it, because a security policy that overstates
its protections is itself a risk.

**A parser fed hostile input.** Recordings come from cameras nobody controls and from files a person was
sent. The box reader is the whole attack surface, so it treats almost every box as opaque bytes, forbids
unsafe code, refuses the panicking constructs, and is fuzzed from M1. Bytes that are never parsed cannot be
parsed wrongly.

**A source that must not be touched.** No surface ever opens a recording for writing. From M2 a guarantee
proves it by digesting and timestamping sources before and after a scripted session on every surface, and
the reader type has no write capability to begin with.

**A signature that must mean something.** Signing is the last step, after the container is finalised, because
any later rewrite invalidates the binding. The signing key has one custodian, the service, since a key inside
a downloadable application is extractable from every copy of it. What that signature attests, and what it
does not, is stated in the interface rather than left to inference
([ADR-0009](docs/adr/0009-sign-last-and-a-single-key-custody.md)).

Enforced today, each by a test in the guarantee suite:

- Every workflow action is pinned to a commit, so a moved tag cannot substitute code into a pipeline.
- No unsafe code exists anywhere, and no library or binary code may panic.
- No tracked file carries a path, an address or a private key from a development environment.
- The dependency policy admits no reciprocal licence, no unknown registry and no git source, and the lock
  file is committed.

Committed, with the milestone that enforces each: [`docs/guarantees.md`](docs/guarantees.md).

## Dependencies

Every version is exact and the lock file is committed, so a build of a given commit resolves the same graph
every time. Updates arrive as pull requests and are gated by the same sequence as any other change.
Advisories, licences, sources and duplicate versions are checked on every run, and an ignored advisory
carries a written reason and a date.
