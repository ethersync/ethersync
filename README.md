# Ethersync

Ethersync enables real-time co-editing of local text files. You will be able to use it for pair programming or note-taking, for example.

Currently, we have a **simple working prototype**, but we're still near the beginning of development.
Thus be warned, that everything is in flux and can change/break/move around quickly.

Currently, we only allow collaborating on one single file at a time. Multi-file collaboration will be one of the next steps.

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
To confirm that worked, try running it:

```bash
ethersync -V
```

should result in:

```
ethersync 0.1.0
```

### Neovim Plugin

Install the [plugin](./vim-plugin) using your favorite plugin manager. For now, use the path to the `vim-plugin` directory in this repository.
Consult the documentation of your plugin manager on how to do that. Example configuration for [Lazy](https://github.com/folke/lazy.nvim):

```lua
{
    dir = os.getenv("HOME") .. "/path/to/ethersync/vim-plugin",
}
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
        ethersync daemon --file=path/to/playground/file
        ```

        This will print an IP address and port (like `192.168.178.23:4242`), which others can use to connect to you.

    - As a **peer**, specify the IP address and port of the host:

        ```bash
        ethersync daemon --file=path/to/playground/file 192.168.178.23:4242
        ```

3. Finally, open the file in Vim:

    ```bash
    nvim path/to/playground/file
    ```

    If everything went correctly, you should see `Ethersync activated!` in the nvim messages and `Client connection established.` in the logs of the daemon.
    You can now collaboratively edit the file together in real-time!

## Sponsors

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/).

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
