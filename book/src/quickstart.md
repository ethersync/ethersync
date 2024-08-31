# Quickstart

Here's how to try out Ethersync!

## 🖥 Try Ethersync on your own computer

### 1. Create an example project directory

Our current convention is to have a subdirectory called `.ethersync` in an Ethersync-enabled directory. So create them both:

```bash
mkdir -p playground/.ethersync
cd playground
touch file
```

### 2. Start the Ethersync daemon

```bash
ethersync daemon
```

### 3. See changes across editors

Open the file twice, in two terminals:

```bash
nvim file
```

You should see `Ethersync activated!` in Neovim, and two `Client connected` messages in the logs of the daemon.

The edits you make in one editor should now appear in both.

> 💡 **Tip**
>
> If that doesn't work, make sure that there's an `.ethersync` directory in the `playground`, and that the `ethersync` command is in the `PATH` in the terminal where you run Neovim.

## 🧑‍🤝‍🧑 Invite other people

If a friend now wants to join the collaboration from another computer, they need to follow these steps:

### 1. Prepare the project directory

```bash
mkdir -p playground/.ethersync
cd playground
```

### 2. Receive the information required to connect

You need to know two things:

- When the other daemon started, it printed a connection address like `/ip4/192.168.23.42/tcp/4242/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv`. If you're in the same local network, you can just use that address. If you're in another local network, see [these instructions](./use-cases/pair-programming.md).
- When the other daemon started, it generated a new passphrase, and printed it in the logs. Only people who know that passphrase are allowed to connect via the network.

## 3. Start the daemon

The command for joining another peer will look something like this:

```bash
ethersync daemon --peer /ip4/192.168.23.42/tcp/4242/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv --secret your-secret-here
```

## 3. Start collaborating in real-time!

If everything worked, connected peers can now open, edit, create and delete files in the shared directory, and the changes will be transferred over!
