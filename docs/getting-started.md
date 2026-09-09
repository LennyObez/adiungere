# Getting started

What you need, how to build it, and how to run every gate the pipeline runs.

## What you need

| Tool | Why | How |
|---|---|---|
| Rust, at the pinned version | The core, the command line and the guarantee suite | Installed automatically by `rustup` from `rust-toolchain.toml` |
| `git` | The guarantee suite lists tracked files through it | Your package manager |
| `cargo-deny` | Advisories, licences, sources and bans | `cargo install cargo-deny --locked` |

Nothing else. There is no media framework to install, no system library to hunt down, and no external media
tool needed to build or test what exists today.

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
