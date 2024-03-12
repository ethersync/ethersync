---
status: accepted
date: 2024-02-29
---
# Which CRDT library to use for the Rust-based Daemon

## Context and Problem Statement

We have already decided to use the CRDT approach in the prototype, but for the next version we want to port things to Rust. So we need a library that supports CRDTs in Rust.

In this ADR, "non-CRDT" means that there are external processes that don't directly interact with the CRDT data structures.
Main one is the editor, which has a plugin that interacts with the deamon without having to know about the CRDT-based approach.

## Decision Drivers

This is a "which library do we choose" decision, but a core one. There are general drivers and specific ones.
First, the general drivers:
* well maintained
* stable / production ready
* well documented
* "ergonomic" API (the interface is easy to understand and to use)
* how is the community around the library?

The specific drivers are:
* Has support for serialization & synchronization built in (s.t. we don't have to roll our own protocol to transfer changes)
* From an incoming change (might be an optimized internal format) we can compute a patch/diff to send to the (non-CRDT) editor.
* (optional) JavaScript support, in case we decided to create a web-based client.
* (optional) Undo/Redo
* (optional) Awareness

## Considered Options

* Automerge
    * [Low level](https://docs.rs/automerge/0.5.7/automerge/)
    * [Higher level](https://github.com/automerge/autosurgeon)
    * Inofficial "automerge-persistent" crate
* [diamond-types](https://github.com/josephg/diamond-types)
* [Yrs](https://github.com/y-crdt/y-crdt)
    * [Y-octo](https://github.com/y-crdt/y-octo)
* [Rust CRDTs](https://github.com/rust-crdt/rust-crdt)

## Decision Outcome

Chosen option: "Automerge", because it comes out best (see below).

An alternative would have been Yrs, it would probably be equally good.
The final decision between the two was mostly based on subtle things
like feeling closer to the community and some risk around the thread-safety with Yrs.

<!-- This is an optional element. Feel free to remove. -->
## Pros and Cons of the Options

### Automerge

General:
* well maintained
    * good ✅
* stable / production ready
    * good ✅
* well documented
    * ✅ makes a mostly good impression, but there might be some outdated and missing parts for the Rust specific parts.
* "ergonomic" API (the interface is easy to understand and to use)
    * almost bad? It's quite an extensive API and that makes it a bit complex and we'd need to invest in learning it.
        * For example there are multiple "levels of abstraction" available and it's not yet clear which one we would want to operate on.
        * This might be, for our use-case, accidental complexity rather than essential, because we don't need all the power that 
* how is the community around the library?
    * ✅ active and friendly community

Specific:
* ✅ Has support for serialization & synchronization built in (s.t. we don't have to roll our own protocol to transfer changes)
    * We can choose if we want to manage transactions ourselves
* ✅ From an incoming change (might be an optimized internal format) we can compute a patch/diff to send to the (non-CRDT) editor.
    * works with something called PatchLog
* ✅ (optional) JavaScript support, in case we decided to create a web-based client.
* ❎ (optional) Undo/Redo
    * it's in [planning](https://github.com/automerge/automerge/issues/58)
    * [undo/redo Master Thesis](https://munin.uit.no/bitstream/handle/10037/22345/thesis.pdf)
* partial (optional) Awareness
    * There's the concept of a cursor, but we don't get full support for awareness metadata like in Yrs.



### diamond-types
General:
* well maintained
    * ❔ hard to say, there's not a lot going on. 9 open and 4 closed issues, but 4 open and 22 closed PRs
* stable / production ready
    * ❎ doesn't feel production ready, even though most features that we need could be available (see specific)
        * e.g. there's no version numbering established
* well documented
    * ❎ there's no documentation
* "ergonomic" API (the interface is easy to understand and to use)
    * ✅ the API is minimalistic and seems elegant
* how is the community around the library?
    * ❎ only developed by one person who doesn't have it as main priority.

Specific:
* ❎ Has support for serialization & synchronization built in (s.t. we don't have to roll our own protocol to transfer changes)
    * not _yet_?
* ❎ From an incoming change (might be an optimized internal format) we can compute a patch/diff to send to the (non-CRDT) editor.
* ✅ (optional) JavaScript support, in case we decided to create a web-based client.
    * JavaScript support via WASM is offered
* ❎ (optional) Undo/Redo
* ❎ (optional) Awareness
    * There's the concept of a cursor, but we don't get full support for awareness metadata like in Yrs.

### Yrs
General:
* well maintained
    * good ✅
* stable / production ready
    * good ✅
* well documented
    * good ✅
* "ergonomic" API (the interface is easy to understand and to use)
    * Still a lot of complexity, but a bit less complexity than automerge
        * Could also just feel easier, because we had used Yjs for the prototype
* how is the community around the library?
    * ✅ Seems [active](https://yjs.dev/#community)

Specific:
* ✅ Has support for serialization & synchronization built in (s.t. we don't have to roll our own protocol to transfer changes)
    * There seem to be syncing-crates
    * Less elegant when compared to automerge
* ✅ From an incoming change (might be an optimized internal format) we can compute a patch/diff to send to the (non-CRDT) editor.
* ✅ (optional) JavaScript support, in case we decided to create a web-based client.
* ✅ (optional) Undo/Redo
* ✅ (optional) Awareness

Something else that *might* be bad, is the [critique that Yrs is not thread-safe and `panics` a lot](https://github.com/y-crdt/y-octo/blob/main/y-octo-utils/yrs-is-unsafe/README.md).
* it's not clear whether this problem would affect our usage
* still, we also very briefly looked at Y-octo, which is a port of Yrs that claims to solve this problem.
    * we didn't try it, because there's no documentation or examples available :|

### Rust CRDTs

We didn't look at this in detail. It was a too low-level approach for us.

## More Information

The decision was done after looking at each library for at least a bit, and we tried most of them.
Our experiments with the different libraries can be found [here](https://github.com/ethersync/automerge-playground)
(see `./src/bin/` for non-automerge code).

It's worth noting that we are planning to write our software in a way that encapsulates the library,
such that we don't commit to a single library forever. This allows us to have internal code that tries
to be independent of the concepts of the selected library (automerge) and also takes off a little bit of
pressure that weighs on the decision: The risk is a bit reduced, we might be able to switch if it turns out
automerge doesn't hold what we expected.
