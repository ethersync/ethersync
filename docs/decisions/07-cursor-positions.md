<!--
SPDX-FileCopyrightText: 2024 blinry
SPDX-FileCopyrightText: 2024 zormit

SPDX-License-Identifier: AGPL-3.0-or-later
-->

---
status: accepted
date: 2024-06-27
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

Chosen option: "Store user state in automerge", because
it is (was) easy to build and we think it will be easy to change if necessary.

## Pros and Cons of the Options

### Store user state in a map in the main Automerge document

The Automerge document for an Ethersync project could be split into a `files` key (which contains the file contents like before), and a `peers` key (which stores information about the peers' state, like cursor positions).

* Good, because transmiting this information would be trivial, with no work required from us at all. The Automege sync protocol handles it for us.
* Bad, because this "ephemeral" data would become part of the CRDT, that each peer needs to persist. The persisted data would be larger as a result, and contain data that's completely irrelevant after a short amount of time.
* Neutral, because it's unclear how to mark user state as "stale". Each peer could delete states after they haven't been updated after (for example) 30 seconds, but could this lead to "fights" between peers that have a connection to the peer, and ones that don't. And if we put the "last seen timestamp" in the CRDT, we assume that all daemons have the same wall time, which isn't necessarily true…

Ideally, user states that have been overwritten anyway could be truncated in the Automerge history, but I don't think that's possible yet.

Because Automerge uses "last writer wins" conflict resolution, the latest peer state should eventually end up in all peers.

### Gossip the user states around, and store them in last-write-wins registers

This would be the appraoch that automerge-repo and Y.js use. Peers send their state information to their connected peers, along with a session ID/user ID and an increasing sequence number, allowing the peers to discard messages they've already seen (and thus, avoiding endless loops). All peers would forward received state messages to all of their other peers, to make sure that the information is available in the whole network.

* Good, because no data accumulates in some CRDT that needs to be persisted forever.
* Bad, because it's more work for us to implement this: It requires more code, and introduces more complexity.

Potentially, we could re-use a library like the [crdt crate](https://docs.rs/crdts)?

## More Information

Automerge explains their reasoning between the session IDs [in this comment](https://github.com/automerge/automerge-repo/blob/caf338c97d8c2c669870e3d3efc34e0eabf3ca60/packages/automerge-repo/src/network/messages.ts#L34-L44).

Y.js's awarenes handling is implemented [here](https://github.com/yjs/y-protocols/blob/master@%7B2024-06-21T09:06:19Z%7D/awareness.js).
More info about the API can be found [here](https://docs.yjs.dev/api/about-awareness).

It's noticeable that both approaches have a general, unstructured "data" field for exchanging this ephemeral information, instead of defining "cursor" or "name" fields. Maybe we should also keep it general like that in the daemon-to-daemon communication, to allow later additions more easily (like user colors, for example).
