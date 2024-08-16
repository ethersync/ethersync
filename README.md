# 🍃 Ethersync

Ethersync enables real-time co-editing of local text files. You can use it for pair programming or note-taking, for example! Think Google Docs, but from the comfort of your favorite text editor!

> [!CAUTION]
> The project is under active development right now. Everything might change, break, or move around quickly.

## Current Features

- 👥 Real-time collaborative text editing
- 📍 See other people's cursors
- 🗃️ Work on entire projects
- ✒️ Local-first: You always have full access, even offline
- 🇳 Fully-featured Neovim plugin
- 🧩 Simple protocol for writing new editor plugins
- 🌐 Peer-to-peer connections, no need for a server
- 🔒 Encrypted connections secured by a shared password

## Planned features

- 🪟 VS Code plugin
- 🔄 Individual undo/redo (we probably won't work on this soon)

## Installation

### 😈 Daemon

Every participant needs a **daemon**, that runs on their local machine, and connects to other peers.
You might be able to use one of the following packages, or you could try a manual installation.

> [!TIP]
> You can use the Nix package on any Linux or MacOS system!

<details>
  <summary>Arch Linux</summary>
  <br>

  Install the [ethersync-git](https://aur.archlinux.org/packages/ethersync-git) package from the AUR.
</details>

<details>
  <summary>Nix</summary>
  <br>
  This repository provides a Nix flake. You can put it in your PATH like this:

  ```bash
  nix shell github:ethersync/ethersync
  ```

  If you want to install it permanently, you probably know what your favorite approach is.
</details>

<details>
  <summary>Manual installation</summary>
  <br>

  You will need a [Rust](https://www.rust-lang.org) installation. You can then compile the daemon like this:

  ```bash
  git clone git@github.com:ethersync/ethersync
  cd ethersync/daemon
  cargo build --release
  ```

  This should download all dependencies, and successfully compile the project.

  For the next steps to succeed you need to make sure that the resulting `ethersync` binary is in your shell PATH.
  One option to do this temporarily is to run this command in the terminal:

  ```bash
  export PATH="$PWD/target/release:$PATH"
  ```
</details>

To confirm that the installation worked, try running:

```bash
ethersync
```

This should show the available options.

### 🇳 Neovim Plugin

You will also need an **editor plugin** connect to the daemon, send it what you type, and receive other peoples' changes.
Right now, we are offering a Neovim plugin. More plugins are planned.

> [!IMPORTANT]
> The plugin currently requires Neovim v0.10.

Again, we have several options of how to install it:

<details>
  <summary>Lazy plugin manager</summary>
  <br>

  If you're using [Lazy](https://github.com/folke/lazy.nvim), you can use a configuration like this:

  ```lua
  {
      "ethersync/ethersync",
      config = function(plugin)
          -- Load the plugin from a subfolder:
          vim.opt.rtp:append(plugin.dir .. "/vim-plugin")
          require("lazy.core.loader").packadd(plugin.dir .. "/vim-plugin")
      end,
      keys = { { "<leader>j", "<cmd>EthersyncJumpToCursor<cr>" } },
      lazy = false,
  }
  ```
</details>

<details>
  <summary>Nix</summary>
  <br>

  For testing purposes, you can run an Ethersync-enabled Neovim like this:

  ```bash
  nix run github:ethersync/ethersync#neovim-with-ethersync
  ```
</details>

<details>
  <summary>Manual installation</summary>
  <br>

  If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

  If you don't already have the repo (i.e you choose a packaged option above):
  ```bash
  git clone git@github.com:ethersync/ethersync
  ```

  Link to the plugin directory from nvim:
  ```bash
  mkdir -p $HOME/.local/share/nvim/site/pack/plugins/start
  cd ethersync # make sure you're in the root of the project
  ln -s $PWD/vim-plugin $HOME/.local/share/nvim/site/pack/plugins/start/ethersync
  ```
</details>

To confirm that the plugin is installed, try running the `:EthersyncInfo` command in Neovim.

## Getting started

To collaborate on a directory called `playground`, follow these steps:


### 1. Create an "Ethersync-enabled" directory

Our current convention is to have a subdirectory called `.ethersync` in an Ethersync-enabled directory. Create it like this:

```bash
mkdir -p playground/.ethersync
cd playground
touch file
```

### 2. Start the daemon

In a group, one person needs to start the session, and the others connect to it.

- As the **starting peer**, run:

    ```bash
    ethersync daemon
    ```

    This will print a connection address (like `/ip4/192.168.23.42/tcp/4242/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv`) which others in the same local network can use to connect to you. (See the FAQ below on how to connect from another local network.)

- As a **joining peer**, specify the address of the starting peer:

    ```bash
    ethersync daemon --peer /ip4/192.168.23.42/tcp/4242/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv
    ```

### 3. Start collaborating in real-time!

You can now open, edit, create and delete files in the shared directory, and connected peers will get your changes! For example, open a new file:

```bash
nvim file
```

If everything went correctly, you should see `Ethersync activated!` in Neovim's messages and `Client connection established.` in the logs of the daemon.

> [!TIP]
> If that doesn't work, make sure that there's an `.ethersync` directory in the `playground`, and that the `ethersync` command is in the PATH in the terminal where you run Neovim.

## Usage

### File ownership

When someone makes a change to a file, the daemons of connected peers will usually write that change directly to the disk.

However, once that file has been opened in an editor, that is undesirable – text editors are not happy if you change their files while they're running. So by opening a file in an editor with Ethersync plugin, that editor takes "ownership" of the file – the daemon will not write to them anymore. Instead, it will communicate changes to the editor plugin, which is then responsible for updating the editor buffer.

Once you close the file, the daemon will write the correct content to the file again. This means that, in an Ethersync-enabled directory, **saving files manually is not required, and doesn't have any meaning** – you can do it if you want, but your edits will be communicated to your peers immediately anyway. It's as if your editor immediately auto-saves.

### Configuration files

If you keep starting Ethersync with the same options, you can put those options into a configuration file at `.ethersync/config`:

```ini
secret = <the shared secret>
port = <port for your daemon>
peer = <multiaddr you want to try connecting to>
```

## FAQ

### What does "local-first" mean?

After you've initially synced with someone, your copy of the shared directory is fully independent from your peer. You can make changes to it, even when you don't have an Internet connection, and once you connect again, the daemons will sync in a more or less reasonable way. We can do this thanks to the magic of [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) and the [Automerge](https://automerge.org) library.

### What do you mean by "more or less reasonable" syncing?

The syncing will not always give 100% semantically correct results:

- When two people create a file with the same name at the same time, one of the two copies will win, and the other one will be overwritten. The daemon's log will tell you which copy won. We're planning to give you more choices or make a backup.
- When two people edit the same place of a source code, version control software like Git would show this as a "conflict", and ask you to resolve it manually.
Ethersync, on the other hand, allows the changes to smoothly integrate. The result is like the combination of their insertions and deletions. So the result will not necessarily compile.

However, the syncing should always guarantee that all peers have the same directory content.

### Can I make changes to a shared directory while the daemon isn't running?

Yes. When you start the daemon the next time, it will compare its persisted state to the actual disk content, calculate a diff, and bring the persisted state up to date. This often will be sufficient; but letting the daemon run and actually tracking the changes as you type them will sometimes lead to a more fine-grained, better syncing result.

### Can I edit a file with tools that don't have Ethersync support?

Yes, changes you make will be shared. However, there are fewer "correctness guarantees", especially if you make many edits in rapid progression.

You can also open a file in an editor without Ethersync plugin – if you change a file, and then save it, the edits will be shared. But if someone else has made an edit in the meantime, that edit will currently get lost.

### Can I open the same file in multiple editors at once?

[Not yet.](https://github.com/ethersync/ethersync/issues/63)

### Can one daemon share multiple directories at the same time?

[Not yet.](https://github.com/ethersync/ethersync/issues/134)

### How can I connect to someone in another local network?

For two people in the same network (for example, in the same wi-fi), the connection will just work. For other cases, you'll currenly need to enable port forwarding from your router to your local machine, so that peers can directly connect to you. The easiest option.

### How should I set up Ethersync for a "shared notes" use case?

While in a "pair-programming" use case, all peers will be online at the same time, for shared notes, it is often desirable to allow peers to go offline, and other peers will still get their changes once they connect.

To enable that, our currently proposed solution is to set up a "cloud peer" – an Ethersync daemon running on a public server, which all users connect to. This resembles a server-client architecture, but all peers are essentially equal. Just the topology of the connections is star-shaped.

## Development

If you're interested in building new editor plugins, read the specification for the [daemon-editor protocol](/docs/editor-plugin-dev-guide.md).

For more information about Ethersync's design, refer to the list of [decision records](docs/decisions/).

## Sponsors

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/).

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
