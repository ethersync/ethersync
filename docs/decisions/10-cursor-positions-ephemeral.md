<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

---
status: proposed
date: 2025-06-25
---
# How do we transmit cursor positions?

## Context and Problem Statement

An important feature of collaborative editing is being able to see each other's cursors. How should we implement transmitting them?

## Decision Drivers

Our solution should ideally be:

* Fast
* Correct (cursors never appear at wrong/shifted locations)
* Simple (little protocol overhead, little complexity)
* Take up little disk space

## Considered Options

* Store user state in automerge
* Gossip user states to last-write-wins

## Decision Outcome

Chosen option: "Gossip user states to last-write-wins", because
we suspect that this improves the performance when using the same doc for a longer time.

## Pros and Cons of the Options


## More Information

See ADR 07 for more Information on this decision, which is superseded by this one.
