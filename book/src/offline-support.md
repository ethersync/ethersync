<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Offline Support

> ℹ️ **Note:**
>
> Ethersync only makes use of the features mentioned on this page in a [note-taking](shared-notes.md) context, *not* in a [pair-programming](pair-programming.md) context where Git is used in parallel. In that case, consider Git your "offline support".
> 
> Ethersync detects whether the directory is inside a Git repository with a configured remote, and disables the offline support.

In Ethersync, you can still work on a shared project, even when disconnected from your peers.

Ethersync uses a data structure called "Conflict-free replicated data type" (CRDT) to enable this, specifically, the [Automerge library](https://automerge.org). The CRDT describes the current file contents, and the edits that were made to it, and allows smoothly syncing with other peers later.

## Making changes while disconnected to peers

You can make changes to a project while disconnected from your peers. If the daemon is running, the changes you make to files will already be put into the CRDT as you type them. If you then connect to other peers which worked on the same project, your changes will smoothly be integrated with theirs.

## Making changes while the Ethersync daemon is not running

You can also make changes to a project while the Ethersync daemon is not running! When you start the daemon later, it will compare the file contents with its CRDT state, calculate a diff, and integrate the patches into its CRDT. This means that from Ethersync's perspective *the files are the source of truth*. After Ethersync has been restarted, its CRDT content will exactly match the file content.

## Starting from scratch

Ethersync saves its CRDT state to `.ethersync/doc`. If you ever want to discard that state, you can delete that file. You might want to do this, for example, if you have previously paired on a project with person A, but now you want to *join* a shared session hosted by unrelated person B. Because B's document history has nothing to do with the one you currently have, syncing them will not work. So by deleting `.ethersync/doc`, you can "start from scratch", and join B.

## What do you mean by "more or less reasonable" syncing?

The syncing will not always give 100% semantically correct results:

- When two people create a file with the same name at the same time, one of the two copies will win, and the other one will be overwritten. The daemon's log will tell you which copy won. We're planning to give you more choices or make a backup.
- When two people edit the same place of a source code, version control software like Git would show this as a "conflict", and ask you to resolve it manually.
Ethersync, on the other hand, allows the changes to smoothly integrate. The result is like the combination of their insertions and deletions. So the result will not necessarily compile.

However, the syncing should always guarantee that all peers have the same directory content.
