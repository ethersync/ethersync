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

### 2. Exchange the information required to connect

Your friend will need to know two things:

- When your daemon started, it printed a **connection address ("multiaddress")** like `/ip4/192.168.23.42/tcp/58063/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv`. If your friend is in the same local network, they can just use that address. If they're in another local network, see [these instructions](pair-programming.md).
- When your daemon started, it generated a **secret passphrase**, and printed it in the logs. Only people who know that passphrase are allowed to connect to it via the network.

In order to allow them to connect, we assume that you sent these two things to your friend (if you're not local, a secure channel is recommended).

### 3. Start the daemon

The command for joining another peer will look something like this:

```bash
ethersync daemon --peer <multiaddress> --secret <passphrase>
```

If a connection can be made, both sides will indicate success with a log message "Peer connected" and "Connected to peer" respectively. If you don't see it, double check the previous steps.

### 4. Start collaborating in real-time!

If everything worked, connected peers can now collaborate on existing files through opening them in their editors.
Type somethings and the changes will be transferred over!
If you're on nvim you should also see your peer's cursor.
