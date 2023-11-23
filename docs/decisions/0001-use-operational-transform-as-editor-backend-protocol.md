---
status: accepted
date: 2023-11-23
---
# Using OT as synchronization protocol between local backend and editor plugin for the prototype

## Context and Problem Statement

In the current prototype there is a backend service written in NodeJS and a vim plugin in Lua.
In tests we noticed that there's a need for some synchronization between those two especially.

There are three components:
- a server component (with potential connections to other peers)
    - this is currently the "etherwiki" instance, so other peers can be simulated by editing on the web.
- a local backend component, which is responsible for communicating with the server as well as the editor plugin.
- an editor plugin (in the case of the prototype a vim plugin), responsible for tracking edits of the user.
    - the edits are communicated to the local backend through an IPC socket.

Where does the synchronization take place:
- between local backend and etherwiki (another demo frontend) the edits are synchronized through Yjs CRDT mechanisms.
- the Vim Plugin and the backend are communicating through a hand-crafted "protocol" (very brittle and prototype-y)
  which doesn't have synchronization capabilities.

### Problem Statement
How do we ensure that plugin and server component are synchronized properly, even though there might be latency on the communication between editor plugin and local backend?

<!-- This is an optional element. Feel free to remove. -->
## Decision Drivers

* motivation
    * Currently there are a number of synchronization bugs, we want to prevent those
    * The currently protocol was just there to have "something" in place, and we now want to iterate
      on something improved.
* requirements
    * we want to have something light-weight
    * ideally all synchronization logic is in the local backend, whereas the server and editor plugins are kept light
    * we don't want to interfere with the user during editing
… <!-- numbers of drivers can vary -->

## Considered Options

* Using an OT-inspired mechanism
* Using CRDTs in the editor plugin as well
* Using some kind of locking mechanism
* … <!-- numbers of options can vary -->

## Decision Outcome

Chosen option: "OT-mechanism", because
it seems to be the only option that is lightweight and doesn't interfere with user editing.

<!-- This is an optional element. Feel free to remove. -->
## More Information

This decision is not set in stone, but is just a first iteration on something that is rather broken.
When the implementation turns out to be still buggy or having other issues that we didn't take into account,
we can iteration once more.
