<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# First steps

Here's how to try out Teamtype!

## 🎬 1. Share a directory

In an empty directory, run:

```bash
teamtype share
```
This will print a "join code" that others can use to connect.

Open a file in the project directory:

```bash
nvim file
```

You should see `Teamtype activated!` in Neovim, and a `Client connected` message in the logs of the daemon.


## 🧑‍🤝‍🧑 2. Join the directory

You can try this on your local computer with a different directory, but it also works over the network!

In another empty directory run the join command that the first daemon printed:

```bash
teamtype join 3-exhausted-bananas
```

Both sides will indicate success with a log message "Peer connected" and "Connected to peer" respectively.

Connected peers can now collaborate on existing files by opening them in their editors.
Type something and the changes will be transferred over!
You should also see your peer's cursor.

## Troubleshooting

### Opening files in Neovim doesn't show a "Client connected" message in the logs of the daemon.

Make sure that the `teamtype` command is in the `PATH` in the terminal where you run Neovim.
