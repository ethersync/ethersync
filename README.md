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

## Planned features

- 🪟 VS Code plugin
- 🔒 Basic authentication
- 🔄 Individual undo/redo (we probably won't work on this soon)
- 🌐 Peer-to-peer connections, no need for a server

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
  export PATH="$HOME/path/to/ethersync/daemon/target/release:$PATH"
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
  nix run github:ethersync/ethersync#neovim
  ```
</details>

<details>
  <summary>Manual installation</summary>
  <br>

  If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

  ```bash
  git clone git@github.com:ethersync/ethersync
  mkdir -p $HOME/.local/share/nvim/site/pack/plugins/start
  ln -s ethersync/vim-plugin $HOME/.local/share/nvim/site/pack/plugins/start/ethersync
  ```
</details>

To confirm that the plugin is installed, try running the `:EthersyncInfo` command in Neovim.

## Usage

To collaborate on a directory called `playground`, follow these steps:


### 1. Create an "Ethersync-enabled" directory

Our current convention is to have a subdirectory called `.ethersync` in an Ethersync-enabled directory. Create it like this:

```bash
mkdir -p playground/.ethersync
```

### 2. Start the daemon

In a group, one person needs to "host" the session, while the others join it. (Peer-to-peer support is planned.)

- As the **host**, run:

    ```bash
    ethersync daemon path/to/playground
    ```

    This will print an IP address and port (like `192.168.178.23:4242`), which others can use to connect to you. (It prints the local and public IP address. Right now, if you want others to be able to join you from outside your local network, you might need to configure your router to enable port forwarding to your computer. A more convenient way to do that is planned.)

- As a **peer**, specify the IP address and port of the host:

    ```bash
    ethersync daemon path/to/playground --peer 192.168.178.23:4242
    ```

### 3. Start collaborating in real-time!

You can now open, edit, and delete files in the shared directory, and connected peers will get your changes! For example, open a new file:

```bash
nvim path/to/playground/file
```

If everything went correctly, you should see `Ethersync activated!` in Neovim's messages and `Client connection established.` in the logs of the daemon.

> [!TIP]
> If that doesn't work, make sure that there's an `.ethersync` directory in the `playground`, and that the `ethersync` command is in the PATH in the terminal where you run Neovim.

## Development

If you're interested in building new editor plugins, read the specification for the [daemon-editor protocol](/docs/editor-plugin-dev-guide.md).

For more information about Ethersync's design, refer to the list of [decision records](docs/decisions/).

## Sponsors

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/).

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
