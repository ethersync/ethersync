<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# First steps

Here's how to try out Ethersync!

## 🎬 1. Set up a project to share

Our current convention is to have a subdirectory called `.ethersync` in an Ethersync-enabled directory. So create them both:

```bash
mkdir -p playground/.ethersync
cd playground
touch file
```

Then, start the Ethersync daemon in the project directory:

```bash
ethersync share
```
This will print a "join code" that others can use to connect.

## 🖥 2. Try Ethersync on your own computer

First, let's see changes across two different text editors!

Open the file in a new terminal:

```bash
nvim file
```

You should see `Ethersync activated!` in Neovim, and a `Client connected` message in the logs of the daemon.

> 💡 **Tip**
>
> If that doesn't work, make sure that the `ethersync` command is in the `PATH` in the terminal where you run Neovim.

Next, in order to see Ethersync working, you can open the file again in a *third* terminal:

```bash
nvim file
```
The edits you make in one editor should now appear in both!

Note that using two editors is not the main use-case of Ethersync. We show it here for demonstrating purposes.


## 🧑‍🤝‍🧑 3. Collaborate with other people

If a friend now wants to join the collaboration from another computer, they need to follow these steps:

### Prepare the project directory

```bash
mkdir -p playground/.ethersync
cd playground
```

### Start the daemon

To connect, run a command like this, with the "join code" output by the daemon on the first computer:

```bash
ethersync join 3-exhausted-bananas
```

Both sides will indicate success with a log message "Peer connected" and "Connected to peer" respectively. If you don't see it, double-check the previous steps.

### Start collaborating in real-time!

If everything worked, connected peers can now collaborate on existing files by opening them in their editors.
Type somethings and the changes will be transferred over!
You should also see your peer's cursor.
