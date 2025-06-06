<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Using Ethersync for pair programming

One use case for Ethersync is to do a **short collaboration session** on an existing project. You could do this, for example, to pair program with another person, editing code together at the same time.

This would also work for more than two people. One person will start the session, and others can connect to it.

## Step-by-step guide

### 1. Starting conditions

If both people already have a copy of the project, make sure you're on the same state – for example, by making sure that you're on the same commit with a clean working tree. If the joining peer has a different state, those changes will be overwritten.

An alternative is that the joining peer starts from scratch, with an empty directory.

Make sure you're both inside the project directory on the command line.

Also this guide assumes you're in the same local network. For other connections consider reading the section on [connection making](connection-making.md).

### 2. Create the `.ethersync` directory

This is our convention to mark a project as shareable: It needs to have a directory called `.ethersync` in it. So both peers should make sure that it exists:

```bash
mkdir .ethersync
```
Note that this directory, similar to a `.git` directory will *not* be synchronized.

### 3. First peer

To start the session, run:

```bash
ethersync daemon
```

This will print, among other initialization information, the [peer identifier and key](connection-making.md#addressing-and-authenticating-with-the-peer)
) which looks like `429e94b149fc1f7d88b2ce1d46cafe9af220bebf084ee6f8b468918d020e9819#32374e6846573354474e4b504b6742773662724e4f6b5a786c5477624f4a6789`.

You need to share this with the other peers.

### 4. Other peers

To join a session, run:

```bash
ethersync demon --peer
```

This will query the user for the identifier and key combination with "Enter peer:".
This should show you a message like "Connected to peer: ...". The hosting daemon should show a message like "Peer connected: ...".

 When used "in production" it's also possible and recommended to use the [configuration file](configuration.md) to provide the peer identifier and key for encryption.

### 5. Collaborate!

Connected peers can now open files and edit them together. Note the [common pitfalls](workarounds.md).

### 6. Stop Ethersync

To stop collaborating, stop the daemon (by pressing Ctrl-C in its terminal). Both peers will still have the code they worked on, and can continue their work independently.

### 7. Reconnect later

If you later want to do another pairing session, make sure that you understand Ethersync's [offline support](offline-support.md) feature and the [local first](local-first.md) concept. When you re-start Ethersync, it will scan for changes you've made in the meantime, and try to send them to the other peer. It is probably safest if you delete the CRDT state in `.ethersync/doc` as a joining peer. The hosting peer doesn't need to do that, it will simply update their state to the latest file content and share that with others.
