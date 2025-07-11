<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# 🍃 Ethersync

Ethersync enables real-time co-editing of local text files. You can use it for pair programming or note-taking, for example! Think Google Docs, but from the comfort of your favorite text editor!

> [!CAUTION]
> The project is under active development right now. Everything might change, break, or move around quickly.

## Features

-   👥 Real-time collaborative text editing
-   📍 See other people's cursors
-   🗃️ Work on entire projects
-   🛠️ Sync changes done by text editors and external tools
-   ✒️ Local-first: You always have full access, even offline
-   🇳 Official plugins for Neovim and VS Code
-   🪟 VS Code plugin
-   🧩 Simple protocol for writing new editor plugins
-   🌐 Peer-to-peer connections, no need for a server
-   🔒 Encrypted connections secured by a shared password

## Install the daemon

<details>
<summary>Arch Linux</summary>
<br>
Install the [ethersync-bin](https://aur.archlinux.org/packages/ethersync-bin) package from the AUR.
</details>

<details>
<summary>Nix</summary>
<br>
This repository provides a Nix flake. You can temporarily put it in your `PATH` like this:

```bash
nix shell github:ethersync/ethersync
```

If you want to install it permanently, you probably know what your favorite approach is.
</details>

<details>
<summary>Binary releases</summary>
<br>
The releases on GitHub come with [precompiled static binaries](https://github.com/ethersync/ethersync/releases/latest) for Linux and macOS. Download one and put it somewhere in your shell's [`PATH`](https://en.wikipedia.org/wiki/PATH_(variable)), so that you can run it with `ethersync`.
</details>

<details>
<summary>Via Cargo</summary>
<br>
If you have a [Rust](https://www.rust-lang.org) installation, you can install Ethersync with `cargo`:

```bash
cargo install ethersync
```
</details>

## Basic usage

In one directory:

```
$ ethersync share

    To connect to you, another person can run:

    ethersync join 5-hamburger-endorse

Peer connected: adfa90edd932732ddf242f24dc2dcd6156779e69966d432fcb3b9fe3ae9831ab
```

In another directory (this can be on another computer!):

```
$ ethersync join 5-hamburger-endorse

Derived peer from join code. Storing in config (overwriting previous config).
Storing peer's address in .ethersync/config.
Connected to peer: 5e6b787fff79074735eb9b56939269100de1e37bc7f7a4d29c277cc24f7ee53d
```

The directories are now connected, and changes will be synced instantly. You can open text files (using editors with an Ethersync plugin), and start collaborating in real time! :sparkles:

## Documentation

**Learn how to install, use, and understand Ethersync in [the documentation](https://ethersync.github.io/ethersync).**

## Development

If you're interested in building new editor plugins, read the specification for the [daemon-editor protocol](https://ethersync.github.io/ethersync/editor-plugin-dev-guide).

For more information about Ethersync's design, refer to the list of [decision records](docs/decisions/).

## Funded by

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/) in 2023/24.

Thanks to the [Prototype Fund](https://www.prototypefund.de/) and the [Federal Ministry of Research, Technology and Space](https://www.bmbf.de/EN/) for funding this project in 2025.

<a href="https://nlnet.nl/"><img src="https://upload.wikimedia.org/wikipedia/en/a/a4/NLnet_Foundation_logo.svg" alt="Logo of the NLnet Foundation" style="height: 80px;"></a>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="https://prototypefund.de/en/"><img src="https://upload.wikimedia.org/wikipedia/commons/1/10/PrototypeFund_Logo.svg" alt="Logo of the Prototype Fund" style="height: 100px;"></a>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="https://okfn.de/en/"><img src="https://upload.wikimedia.org/wikipedia/commons/4/4d/Open_Knowledge_Foundation_Deutschland_Logo.svg" alt="Logo of the Open Knowledge Foundation Germany" style="height: 100px;"></a>
&nbsp;&nbsp;
<a href="https://www.bmbf.de/EN/"><img src="https://upload.wikimedia.org/wikipedia/commons/d/df/BMFTR_Logo.svg" alt="Logo of the German Federal Ministry of Research, Technology and Space" style="height: 110px;"></a>

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This project is [REUSE](https://reuse.software) compliant, see the headers of each file for licensing information.
