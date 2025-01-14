<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

---
status: accepted
date: 2024-02-08
---
# Client-side process structure

## Context and Problem Statement

Ethersync needs processes on the client device, to sync up with other nodes, and to communicate with the editors.

How many processes should there be, and how should the communication on a device be structured?

## Decision Drivers

* We want to make editor plugins as simple as possible.
* We want to be flexible, to support different ways to connect (maybe peer-to-peer daemons in the future?).
* We want simple, maintainable code.

## Considered Options

* Central daemon
* One daemon per synced repository
* One daemon per editor session

## Decision Outcome

Chosen option: "Central daemon", because it makes the implementation of editor plugins as simple as possible.

This complicates the implementation of the daemon somewhat, because it needs to "dispatch" incoming messages to different directory handlers internally, but this seems worth the tradeoff.

## Pros and Cons of the Options

### Central daemon

There is a single daemon process on the client device, which is responsible for all synced directories. Editors never spawn new daemons, they just talk to the central daemon.

The central daemon needs some kind of protocol to initiate new shares. For example, there could be a {"method": "request-init", "params": {"path": "path/to/dir"}} message type.

The editors (or the sidecar processes) can find the daemon via a fixed soket path like `/var/user/1000/ethersync`.

* Good, because there's just one single process to manage (plus potentially "sidecar processes", see "More Information")
* Neutral, because it makes the daemon-client protocol a bit more complicated (initiating new shares, for example). But this complexity exists anyway.

### One daemon per synced repository

This is the approach we used in the first prototype. There needs to be a daemon process per synced repository. They all work very similarly. These daemons work independent of each other anyway.

If you want to start a new collaborative session from within an editor, the editor needs to spawn the new daemon somehow.

How do editors find the correct daemon? Maybe use UNIX domain socket `.ethersync/socket` as connection point. Is that cross-platform?

* Good, because the daemons can be built a bit simpler. They have a single directory to take care of, and don't need to dispatch to different "sub-systems".
* Bad, because editors need to do more work: They need to find the correct daemon, and send messages to it. Potentially, they need multiple connections (if you open files from multiple synced shares).

### One daemon per editor session

This is how LSPs usually seem to work. Once an editor is started, it kicks off its own LSP process, and connects with it using stdin/stdout. That process will be closed together with the editor. In Ethersync, there would be one daemon per opened editor session.

What happens when one editor opens files from multiple shared directories?

* Good, because it's extremely simple.
* Bad, because when the same file is opened in multiple editors, it seems difficult to coordinate them. This is a strong reason not to use this option.
* Bad, because in this setting, there will be no sync without an opened editor. We want Ethersync to work with other command line tools that modify the data.

## More Information

### Prior work

How does other software solve daemonization?

- gopls <https://go.googlesource.com/tools/+/refs/heads/master/gopls/doc/daemon.md>

    Has a daemon process and "sidecar" processes:

        gopls serve -listen="unix;/tmp/golps" -rpc.trace

    and

        gopls -remote="unix;/tmp/golps"

- clangd <https://clangd.llvm.org/design/>

    daemon-side: One process per file, which run "clang in a loop"

- ra-multiplex <https://github.com/pr2502/ra-multiplex>

    Share a single rust-analyzer server between LSP clients. Program has `ra-multiplex server` and `ra-multiplex client` commands.

Many software seems to put UNIX sockets in /run/user/<user-id> or /run directly.

### Editor-to-daemon connection

A related, but independent question: How should editors communicate with the daemon processes? There seem to be at least three options:

1. Using a "connector/sidecar process" (started by `ethersync editor-connect`, for example), which each editor spawns itself, and then communicates with it via stdin/stdout. This is similar to how LSPs are spawned. The connector processes would then connect with the respective daemon process using a socket connection, and just pass messages through.
2. Directly using a Unix domain socket connection.
3. Directly using a TCP connection.

All approaches seem functionally equivalent. The last two options require that all editors have the ability to make that type of connection. Potentially, daemons could offer both forms to connect. We lean towards option 1.

Sidecar processes find main the process via a fixed socket path or a fixed port.
