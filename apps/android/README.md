# Android

> **Empty until M7.** What follows describes what will live here and why, not what is here now.

The Android application: a declarative interface on the current design language, over the Rust core reached
through a generated foreign function binding.

Three things make this surface different from the others.

**Track selection is always explicit.** The platform player exposes each video track of the recording as its
own group, so choosing the rear camera is an override the application states, never a default it inherits. A
guarantee will refuse an implicit selection, because a default that silently changed would put the wrong
camera in front of someone assembling a claim.

**The library is reached through the picker first.** The picker needs no permission and returns what a person
chose. The broad media permission is an optional accelerator that the store has to approve, and until that
approval is measured the application claims nothing about scanning a whole library. Whether a cloud-only
video hands back its original bytes is a separate open question, and until it is answered the product makes
no claim about cloud libraries at all.

**The dual view is measured before it is offered.** Two simultaneous decodes plus a composite are not
guaranteed on any device. The second backend is experimental upstream, so the view sits behind a flag with
two measured implementations rather than one assumed to work.

Two flavours are built: one for the store, and one that resolves no proprietary service anywhere in its
dependency graph, checked by a guarantee rather than by intention.

Populated in **M7**.
