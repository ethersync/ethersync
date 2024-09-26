# First steps

Here's how to try out Ethersync!

## 🖥 Try Ethersync on your own computer

### 1. Create an example project directory

Our current convention is to have a subdirectory called `.ethersync` in an Ethersync-enabled directory. So create them both:

```bash
mkdir -p playground/.ethersync
cd playground
touch file
```

### 2. Start the Ethersync daemon in the project directory

```bash
ethersync daemon
```

You should see some log output indicating that things are initialized etc.

### 3. See changes across editors

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


## 🧑‍🤝‍🧑 Invite other people

If a friend now wants to join the collaboration from another computer, they need to follow these steps:

### 1. Prepare the project directory

```bash
mkdir -p playground/.ethersync
cd playground
```

### 2. Start the daemon

The first daemon will print the full command required to connect to it in its logs.
It will look like this:

```bash
ethersync daemon --peer <multiaddress>
```

The daemon might print lines like this multiple times.
If you're in the same local network, the one starting with "192.168" is probably the right one.
If you're in different local network, see [these instructions](pair-programming.md).

If a connection can be made, both sides will indicate success with a log message "Peer connected" and "Connected to peer" respectively. If you don't see it, double check the previous steps.

### 3. Start collaborating in real-time!

If everything worked, connected peers can now collaborate on existing files by opening them in their editors.
Type somethings and the changes will be transferred over!
You should also see your peer's cursor.
