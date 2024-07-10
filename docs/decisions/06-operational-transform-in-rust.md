<!--
SPDX-FileCopyrightText: 2024 blinry
SPDX-FileCopyrightText: 2024 zormit

SPDX-License-Identifier: AGPL-3.0-or-later
-->

---
status: accepted
date: 2024-03-12
---
# Which Operational Transformation Library to use in Rust?

## Context and Problem Statement

In [ADR 01](./01-use-operational-transform-as-editor-backend-protocol.md) we have decided to use Operational Transformation between local backend and editor plugin for the prototype.

For the next iteration, unless we're hit by an epiphany and realize it'll not be necessary, we'll use a similar approach.
This backend will be written in Rust, so we'll need a library that does similar things as https://www.npmjs.com/package/ot-text-unicode does.

## Decision Drivers

This is another "library choosing" ADR. So similarly to [ADR 05](./05-crdt-library.md), the general drivers are:
* well maintained
* stable / production ready
* well documented
* "ergonomic" API (the interface is easy to understand and to use)
* how is the community around the library?

It's worth noting, that it's a much smaller feature (we could even try to implement it ourselves), thus we don't need to find the perfect solution here and the above points are not that important factors.
None of the considered option could be considered well maintained, stable and with a community... but that could still be okay in this scenario.
We'll try the first best one and if it doesn't work, we'll reconsider or even solve this problem ourselves.

As for specific features we need, it's also straightforward:
* Unicode supported Operational Transformation
* (optional) being conceptually similar to the NodeJS package we used


## Considered Options

* https://crates.io/crates/operational-transform
* https://github.com/josephg/textot.rs
* https://crates.io/crates/kyte

## Decision Outcome

Chosen option: "operational-transform", because it combines the advantages of being on crate.io, as well as allowing to pass function parameters by reference.

## Pros and Cons of the Options

### operational-transform

* Good, because it's the most downloaded on crates.io (swarm intelligence bonus :D)
* Neutral-good, because it supports Serde serialization
* Bad, because it's two years old
* Bad, because it doesn't seem to be fuzz-tested

### textot.rs

* Good, because it's definitely feature compatible to the NodeJS package,
    as it's by the same author and meant to be compatible.
* Neutral to bad, because it's not on crates.io (that might make our life a bit harder?)
    * the package author actually offers to upload it there, so it's a neutral
* Bad, because it's not recently been maintained

### kyte

* Good, because I think the Quill compatible approach is what we need
* Good, because it's fully fuzzer tested (their claim)
* Good, because it's not "years old" as the others, but:
* Bad, because there are only two commits -- does it already do what it promises?
    * maybe it's neutral and this tiny feature doesn't need more?
* Bad, because the API is built in a way where you have to clone many parameters
* Bad, because it doesn't collapse retains that follow each other
