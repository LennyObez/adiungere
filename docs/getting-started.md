# Getting started

What you need, how to build it, and how to run every gate the pipeline runs.

## What you need

| Tool | Why | How |
|---|---|---|
| Rust, at the pinned version, with its formatter, linter and LLVM tools | The core, the command line and the guarantee suite; the LLVM tools carry the symbol reader that judges the release binary | Installed automatically by `rustup` from `rust-toolchain.toml`, components included |
| `git` | The guarantee suite lists tracked files through it | Your package manager |
| `cargo-deny` | Advisories, licences, sources and bans, at the version pinned in `tools/versions.toml`; the gate refuses another | `cargo install cargo-deny --locked --version <the pinned one>` |
| `shellcheck` | The scripts under `scripts/` are checked like any other source | Your package manager |
| `python3` on the pinned line | Runs the reference fingerprint script, which the guarantee suite compares with the product | Your package manager; the line is in `tools/versions.toml` |
| The pinned media tool | Its elementary stream output is what the secondary digest is defined against | `scripts/fetch-tools.sh`, which places it under `.tools/` after checking its digest |

Nothing else. There is no media framework to install and no system library to hunt down; the media tool
is a developer oracle, fetched once and never linked. The gate script refuses to run when one of these is
missing, because a step that did not run is not a step that passed.

The toolchain is pinned patch-exact in one file, at the repository root so that every invocation anywhere in
the checkout resolves to it. Do not install it by hand and do not name a version on a command line: `rustup`
reads the pin, and a guarantee refuses any pipeline that restates it and any second pin file.

## Build and test

```console
$ git clone https://github.com/LennyObez/adiungere
$ cd adiungere/core
$ cargo build --workspace
$ cargo test --workspace
```

The first build downloads the pinned toolchain if you do not have it, which takes a few minutes once.

## Run the whole gate sequence

Never announce a result without the command that produced it. This is that command:

```console
$ scripts/gate.sh
```

It runs the sequence from [`testing.md`](testing.md) in order, writes each step's output to its own file
under an ignored directory, reads each exit code before moving on, and stops at the first red. A scoped run
answers a different question and its green is not the gate's green.

## Inspect a recording

```console
$ cargo build --release -p adiungere-cli
$ target/release/adiungere inspect clip.mp4
$ target/release/adiungere fingerprint clip.mp4 --manifest clip.manifest.json
$ target/release/adiungere verify clip.manifest.json clip.mp4
$ target/release/adiungere report clip.manifest.json
$ target/release/adiungere detect /media/card/ --cache ~/.cache/adiungere-scan.json
$ target/release/adiungere export clip.mp4 --camera rear --out rear.mp4
$ target/release/adiungere export clip.mp4 --camera both --out archive.mp4
$ target/release/adiungere export front.mp4 rear.mp4 --out joined.mp4
$ target/release/adiungere export clip.mp4 --tracks 1,2 --out chosen.mp4 --manifest chosen.json
```

`inspect` reads the headers and the movie box and never the media, so it answers in milliseconds on any
size of recording. `fingerprint` reads every byte. Every number either command prints is defined in
[`integrity.md`](integrity.md) with the command a stranger runs to reproduce it. Add `--format json` to
any of them for one document instead of prose.

`export` never touches the recording it reads. It writes the output under a temporary name, reads it back,
and keeps it only when every track carries its source's fingerprint; the manifest goes beside the output as
`<output>.manifest.json` unless `--manifest` names another place. Progress is printed on the error stream,
and an interruption removes the partial file. With two recordings, the front camera and the audio of the
first and the first video track of the second are joined into one two-track file; with `--camera` or
`--tracks` and one recording, the named cameras or the named track indices are taken.

## Read the evidence register

Every load-bearing assumption in this project is a probe with a verdict. The register is prose you can read
and data you can query.

```console
$ cargo run -p adiungere-cli -- probes list
$ cargo run -p adiungere-cli -- probes list --verdict "not started" --milestone M1
$ cargo run -p adiungere-cli -- probes show P11
$ cargo run -p adiungere-cli -- probes check
```

`probes check` reconciles [`evidence.md`](evidence.md) with [`roadmap.md`](roadmap.md) and fails when a probe
is named in one and missing from the other. It runs in the pipeline for the same reason.

Add `--format json` to any listing to get machine-readable output.

## Where things are

The tree has three tiers: what ships to a person, what the rest of the code consumes, and the supporting
material.

| Path | What is there |
|---|---|
| `apps/` | Everything that ships to a person: `android`, `apple`, `linux`, `site`, `windows` |
| `core/` | The Rust workspace: the command line, the guarantee suite, and the crates that arrive with M1 |
| `web/` | The shared player and export interface in TypeScript, used by `apps/site` and `apps/windows` |
| `design/` | The token source that generates every platform theme |
| `docs/` | Roadmap, architecture, testing, the evidence register and the decision records |
| `scripts/` | The gate runner and the checks that do not belong to a crate |
| `infra/` | How an environment is described and deployed |

Only `apps/site`, `core`, `docs` and `scripts` hold anything today. Every other directory carries a README
naming the milestone that fills it.

## Before you open a pull request

Read [`CONTRIBUTING.md`](../CONTRIBUTING.md). The short version: write the failing test first and watch it
fail, run the whole gate sequence, sign your commits, and remember that every tracked file is published.
