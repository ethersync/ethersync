<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# System overview

Teamtype is a system for real-time local-first collaboration on text files, where
- *real-time* means that edits and cursor movements should appear immediately while you are in a connection with your peer
- *local-first* means that it's also possible to continue working on the project while you're (temporarily) offline

Here's a diagram of the components that are involved:

```
  daemon <---(Internet)---> daemon
  ^    ^                    ^    ^
  |    |                    |    |
  v    |                    v    |
 file  |                   file  |
system |                  system |
   ^   |                      ^  |
   |   |                      |  |
   v   v                      v  v
   editor                    editor
```

## Text editor

Text editors (with an installed Teamtype plugin) is what users most directly communicate with.
If they make a change to a file, the editor instantly communicates every single character edit to the daemon.

The plugins also display other peoples' cursors in real-time.

## Daemon

On each participant's computer, there's a Teamtype daemon, keeping the file's content in a data structure called "CRDT".

The daemon collects changes being communicated by the connected editor, and syncs them with other peers.
If conflicts arise, because two edits happened at the same time, they will be resolved by the daemon automatically.

If the daemon is offline, it records the change locally and will communicate it to the other peers later.

## The project

When collaborating with your peers, we assume that you are working on a set of files which are in a common directory.
We call this directory the *project*.
You can compare it, if you're familiar with that, with a git repository.

The tracking of, and communication about changes happens only inside the realm of that directory
and whatever it contains recursively (which means it includes sub-directories and the files therein).
Most files are synchronized, except for [ignored files](ignored-files.md).

Currently, you will need to start one daemon *per project*.
When you start the daemon, you have the option to provide the directory as an optional `--directory` parameter, for example:

    teamtype share --directory [DIRECTORY]

If you leave it out, the current directory is selected.
