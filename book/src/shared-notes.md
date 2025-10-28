<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Using Teamtype for writing shared notes

Another use case for Teamtype is to have a **long-lasting collaboration session** on a directory of text files (over the span of months or years). This is similar to how you would use Google Docs, Teamtype or Hedgedoc to work on text. It would be suited for groups who want to write notes or documentation together.

This use case is different from the "pair-programming" use case, because there, all peers are online at the same time. When you're working on a directory of notes for a longer time, it might happen that you make a change to a file, and then go offline, while the other peers are also offline. Still, you want other peers to be able to receive your changes.

We suggest to use a ["cloud peer"](connection-making.md#cloud-peer), a peer that is always online.

## Step-by-step guide

You need to have access to a server on the Internet, and install the Teamtype daemon there.

### 1. Set up the directory

On the server, create a new directory for your shared project:

```bash
mkdir my-project
cd my-project
```

### 2. Start the daemon

Launch the daemon in a way where it will keep running once you disconnect from your terminal session on the server. You could use `screen`, `tmux`, write a systemd service, or, in the easiest case, launch it with `nohup`:

```bash
nohup teamtype share --show-secret-address &
```

Check the output of the command (written to the file `nohup.out` when using `nohup`) for the node's secret address.

### 3. Collaborate!

Other peers can now connect to the "cloud peer". It is most convenient for them to put the secret address into their configuration file:

```bash
echo "peer=<secret address>" >> .teamtype/config
```

Then, they can connect anytime using

```bash
teamtype join
```

## Tracking edits over time

To keep track of edits over time, you can use the `bookmark` and `diff` subcommands.

`teamtype bookmark` will remember the current directory contents, so that you can compare to that state later. You can run this before connecting to your peer, for example, so that you can see which changes the peer introduced.

`teamtype diff --tool <tool>` will create snapshots of the bookmark and the current state and opens a diff-viewer of your choice to compare them. A good graphical diff viewer is [meld](https://meldmerge.org/).
