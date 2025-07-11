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

### 2. First peer

To start the session, run:

```bash
ethersync share
```

This will print, among other initialization information, a [join code](connection-making.md#join-codes), which looks like `3-exhausted-bananas`.

You can share this with one other person, to allow them to connect.

### 3. Other peers

To join a session, run a command like this:

```bash
ethersync join 3-exhausted-bananas
```

This should show you a message like "Connected to peer: ...". The hosting daemon should show a message like "Peer connected: ...".

### 4. Collaborate!

Connected peers can now open files and edit them together. Note the [common pitfalls](workarounds.md).

### 5. Stop Ethersync

To stop collaborating, stop the daemon (by pressing Ctrl-C in its terminal). Both peers will still have the code they worked on, and can continue their work independently.

### 6. Reconnect later

Note that if the shared folder is **inside a Git repository with a remote**, your daemon will start a new history every time you start it. This is because when you're using Ethersync in parallel with Git fetches, updating your local files from the previous history doesn't make sense anymore.

If you don't have a Git remote, Ethersync uses the [offline support](offline-support.md) feature. When you re-start Ethersync, it will scan for changes you've made in the meantime, and try to send them to the other peer. If you don't want this, you can delete the CRDT state in `.ethersync/doc` as a joining peer, to receive the history from your peer. The hosting peer doesn't need to do that, it will simply update their state to the latest file content and share that with others.