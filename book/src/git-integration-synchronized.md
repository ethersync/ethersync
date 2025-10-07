<!--
SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Synchronized method

It is possible to tightly integrate Ethersync and Git by setting the `--sync-vcs` flag.

Setting it causes Ethersync to *not* [ignore](ignored-files.md) version-control directories when synchronizing files.
This might seem like an unusual approach to working with Git, but in our tests, when pairing, it seems to work more or less smoothly.

Beware that this is quite **EXPERIMENTAL** and be prepared to potentially loose your data (especially when learning to "think" in this potentially unfamiliar scheme).

We'll describe this method from the perspective of Git, but it might/should work for other version-control systems as well.

## Why would you want this?

Using the synchronized method will synchronize all contents in `.git`:
This includes all commits, branches, tags, objects, the current position of the HEAD.
It will also synchronize `.git/config`, so remotes, and tracked branches will now be the same for all peers.

The effect is surprising:

- When one peer creates a commit, it will immediately be visible for the other peers.
- When one peer changes the branch, it will now also be checked out for the other peers.
- Git's index will have the same state for all peers.

This allows a very "smooth" collaboration, where you don't need to pull or push explicitly.

## Committing together

When one of the participants initiates `git commit`, Git will, depending on your setup, open an editor where you can write the commit message.
The temporary file for this is `git/COMMIT_EDITMSG`, which is now synced, so any peer can open it as well and you can edit it together.
The initiator of the commit has the "power" to finalize the commit by closing the file, their Git will create the commit.

## Recommendation: Use a separate directory to try this

You might not want to share all local branches with your peers.
To have a safe environment to try this type of synchronization, we recommend to not use your regular project directory to collaborate.
Instead, you might want to create a separate directory (`<projectname>-together`?) with a fresh clone.

After the collaboration session, you can push your results to a remote, and pull them in your regular project directory.

## Caveat: Shared local user configuration

When `.git/config` contains settings like `user.name` or `user.email` (because one peer had configured a local Git identity), this might result in an unexpected result:
All peers will have the same shared Git identity when creating commits.
As a workaround, you can either use `git commit --author`, or conditional user configuration depending on the directory, using [`includeIf`](https://git-scm.com/docs/git-config#_includes).
