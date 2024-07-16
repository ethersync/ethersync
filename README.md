# Ethersync

Ethersync enables real-time co-editing of local text files. You will be able to use it for pair programming or note-taking, for example.

Currently, we have a **simple working prototype**, but we're still rather at the beginning of development.
Thus be warned, that everything is in flux and can change/break/move around quickly.
Especially the communication protocols are subject to modifications, so reach out to us if you want to
work on an editor plugin for you favorite editor.
Also the software has some limitations and bugs that might eat your cat.

The current features (including some planned ones marked with the construction emoji) are:

- ✅ Neovim support
  - ✅ collaborative editing
  - ✅ cursor/peer awareness
  - ✅/🚧 Multi-file support
    - ✅ collaborating on a static set of files & directories
    - 🚧 choosing which files are shared (e.g. via .gitignore/--ignore)
    - 🚧 collaborating on a changing set of files (adding/deleting files)
  - ❌ individual undo/redo (we probably won't work on this soon)
- 🚧 VS Code support
- 🚧 Basic authentication

## Components

Ethersync consists of two components:

- Every participant needs a **daemon**, that runs on their local machine, and connects to other peers.
- **Editor plugins** connect to the daemon, send it what you type, and receive other peoples' changes.
  Currently, there's a plugin for Neovim, but other editor integrations are planned.

## Setup

Each participant (one is the **host**, all others are **peers**) need to set up these two components.
First, clone this repository:

```bash
git clone git@github.com:ethersync/ethersync
cd ethersync
```

### Daemon

To install the daemon component, you need a [Rust](https://www.rust-lang.org) installation. You can compile the daemon like this:

```bash
cd daemon
cargo build
```

This should download all dependencies, and successfully compile the project (currently as a debug build, as we're in early development).

For the next steps to succeed you need to make sure that the resulting `ethersync` binary is in your shell PATH.
One option to do this temporarily is to run this command in the terminal:

```bash
export PATH="$HOME/path/to/ethersync/daemon/target/debug:$PATH"
```

To confirm that worked, try running it:

```bash
ethersync
```

This should show the available options.

### Neovim Plugin

- If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

    ```bash
    mkdir -p $HOME/.local/share/nvim/site/pack/plugins/start
    ln -s $HOME/path/to/ethersync/vim-plugin $HOME/.local/share/nvim/site/pack/plugins/start/ethersync
    ```

- If you're using [Lazy](https://github.com/folke/lazy.nvim), you can specify the path to the `vim-plugin` directory in this repository like this:

    ```lua
    {
        dir = os.getenv("HOME") .. "/path/to/ethersync/vim-plugin",
        keys = { { "<leader>ej", "<cmd>EthersyncJumpToCursor<cr>" } },
        lazy = false,
    }
    ```

- For other plugin managers, it's often convenient to provide a Git repository which contains the plugin at the top level.
We manually publish the latest version at <https://github.com/ethersync/ethersync-vim>, so you can specify the repo like this (for example, for [vim-plug](https://github.com/junegunn/vim-plug)):

    ```vim
    Plug 'ethersync/ethersync-vim'
    ```

## Usage

To collaborate on a file called `file` in a directory called `playground`, follow these steps:

1. Right now, our convention to mark an "Ethersync-enabled" directory is that there is a subdirectory called `.ethersync` in it. (A more convenient way to use Ethersync is planned.) So you need to create it:

    ```bash
    mkdir -p playground/.ethersync
    ```

2. After that, start the daemon. In a group, one person needs to "host" the session, while the others join it. (Peer-to-peer support is planned.)

    - As the **host**, run:

        ```bash
        ethersync daemon --file=path/to/playground
        ```

        This will print an IP address and port (like `192.168.178.23:4242`), which others can use to connect to you. (It prints the local IP address by default, but you can also be reached using your public IP address. Right now, you might need to configure the host computer to open the port to the outside. A more convenient way to do that is planned.)

    - As a **peer**, specify the IP address and port of the host:

        ```bash
        ethersync daemon --file=path/to/playground/file 192.168.178.23:4242
        ```

3. Finally, open the file in Vim:

    ```bash
    nvim path/to/playground/file
    ```

    If everything went correctly, you should see `Ethersync activated!` in the nvim messages and `Client connection established.` in the logs of the daemon.
    If that doesn't work, make sure that there's an `.ethersync` directory next to the `file`, and that the `ethersync` command is in the PATH in the terminal where you run Neovim.
    You can now collaboratively edit the file together in real-time!

## Development

If you're interested in building new editor plugins, read the specification for the [daemon-editor protocol](docs/daemon-editor-protocol.md). For more information about Ethersync's design, refer to the list of [decision records](docs/decisions/).

## Sponsors

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/).

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
