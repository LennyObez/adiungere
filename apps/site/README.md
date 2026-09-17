# Site

adiungere.com. Today it serves one static page and a machine-readable security contact. At M4 it becomes the
product: the library, the player, the export dialogue and the check page, all running in the browser with no
recording ever leaving the device ([ADR-0008](../../docs/adr/0008-the-website-runs-in-the-browser.md)).

## What is here

| Path | What it is |
|---|---|
| `public/index.html` | The page served at the root. One file, no build step, no request to any other host |
| `public/.well-known/security.txt` | The machine-readable security contact, as the standard for it defines |

`public/` is the whole published surface. Everything outside it stays in the repository and is never served.

## Why the page looks like nothing in particular

It carries no visual identity, and that is deliberate rather than unfinished. The identity is designed at
M4, from a brief that asks for argued directions and a recommendation. A placeholder that invented a colour
and a typeface would make that decision by default, three milestones early, and the real design would then
be judged against an accident.

So the page uses the reader's own system typeface, follows the light or dark setting they already chose, and
spends its effort on saying honestly what exists and what does not.

## How it is deployed

The host pulls the default branch into a directory outside the served tree, and the domain's document root
points at this directory's `public/`. So what a visitor receives **is** the working copy of a commit whose
signature verifies: nothing is built on the server, nothing is copied, and nothing is uploaded by hand.

That also means the published surface is exactly `public/` and can never accidentally widen. The repository
sits above the document root, so neither the sources nor the version history is reachable over HTTP.

When the site gains a build step at M4, a publish action appears alongside it. There is none today, because a
script nothing runs is a script nobody maintains.

## What the host has to honour

Probe **P19** in [`../../docs/evidence.md`](../../docs/evidence.md) measured the deployment from the outside:
the page is served from the pulled checkout, the repository above the document root is not reachable, and
the response headers are the host's defaults. Two consequences follow for whoever administers the host.

The security contact a visitor receives must be the tracked file in `public/.well-known/`, byte for byte. A
host that generates its own contact at that path publishes a contact this project did not write, with an
expiry this project does not track, and the gate's check of the tracked file then measures the wrong thing.

Whether the host serves WebAssembly with the right media type and long cache lifetimes is probe **P38**,
measured when the first module is published at M4. The answer also decides whether the signing service can
run alongside the site later.
