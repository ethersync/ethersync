---
status: accepted
date: 2024-09-13
---
# How to keep directory structures in sync?

## Context and Problem Statement

Instead of only syncing a static, single file, we want to be able to collaborate on entire directory structures.

What should our core data structure look like, and how should we approach edge cases?

## Decision Drivers

Our chosen solution should:

* Not behave in a surprising way for end users.
* Avoid data loss as much as possible (for example, in the case of a file name conflict).
* Sync without interruptions/without users having to give their input to solve "merge conflicts".

## Considered Options

* Plain keys in one CRDT datastructure
* Virtual CRDT filesystem
* One CRDT document per file

## Decision Outcome

Chosen option: "plain keys in on CRDT", because
it's the simplest solution and we are willing to accept the drawbacks (some risk for conflicts/data loss) instead of taking in complexity.
We'll be tackling the conflicts and preventing data loss by detecting these cases and creating a backup beforehand.

## Pros and Cons of the Options

### Plain keys in one CRDT datastructure

In the CRDT datastructure, we keep a dictionary where the key is the full filename from the root of the shared directory, and the value is the "text"-type CRDT.

* Good, because it is conceptually simple.
* Neutral, because we can detect concurrent file creations of the same name, and work around it.
* Bad, because when a directory is renamed, all sub-files have to be removed and recreated.
* Bad, because problematic case 2 is hard to solve in a satisfying way.

### Virtual CRDT filesystem

Following an approach as described in [this paper](https://inria.hal.science/hal-03278658/document), we model the shared directory as a virtual, CRDT-powered filesystem. Like in a proper file system, its entries would be identified via inodes. All file operations are then directly mapped to an underlying CRDT. Files are identified by path and user ID, which avoids problematic case 1 (see below).

* Good, because it might offer the smoothest and most "correct" handling of concurrent file operations.
* Good, because it avoids problematic case 1. In case of a conflict, both versions are "rendered" to disk smoothly.
* Bad, because it is very complex.

### One CRDT document per file

Every file could be its own CRDT document with an URI. Directories could list to those URIs.

* Good, because problematic case 2 (see below) could be avoided - the edits would just translate over to the new filename.
* Bad, because it is complex, and would require syncing many CRDT documents at once.

## More Information

We think that file operations like creating or deleting a file don't happen very often in our use cases. File edits, however, happen a lot in comparison. So it would be okay if our data structure would neglect the first two operations a bit.

Here's a list of problematic cases that can happen when dealing with directory structures. In each case, we look at two peers, which have done some work in parallel (while offline), and now they try to reconnect.

As a simplification, let's assume that there's no "move/rename" operation, and that those would just be mapped to a deletion and a creation.

### Case 1: Both users create a file of the same name.

What could happen?

- A single file with both contents is created, one after the other.
- One file "wins" the original name, and the other appears with an alternative filepath, like "file.2".

### Case 2: One user edits a file, while the other removes it.

What could happen?

- The edited version stays intact, the deletion is not done.
- The deletion happens, the edited version is written to "file.2".
