<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# 🍃 Ethersync

*Multiplayer mode for your text editor!*

Ethersync enables real-time collaborative editing of local text files. You can use it for pair programming or note-taking, for example. It's the missing real-time complement to Git!

## Features

- 👥 Edit files at the same time, across different text editors
- 📍 See your peers' cursors and selections
- 🗃️ Work on entire projects, the way you're used to
- 🔒 Encrypted peer-to-peer connections, no need for a server
- ✒️ Local-first: You always have full access, even offline
- 🧩 [Simple JSON-RPC protocol](https://ethersync.github.io/ethersync/editor-plugin-dev-guide.html) for writing new editor plugins

## 🚦 Project status

The project is under active development right now. We use it every day, but there's still some [bugs](https://github.com/ethersync/ethersync/issues?q=sort%3Aupdated-desc+is%3Aissue+is%3Aopen+%28label%3Abug+OR+type%3ABug%29) to be aware of.

## 📥 Installation

### 1. Install the `ethersync` command

<details>
<summary>Arch Linux (btw)</summary>
    
```
yay -S ethersync-bin
```
</details>

<details>
<summary>Nix</summary>

This repository provides a Nix flake. To put `ethersync` in your PATH temporarily, run:

```
nix shell github:ethersync/ethersync
```
</details>

<details>
<summary>Binary releases</summary>

The [releases on GitHub](https://github.com/ethersync/ethersync/releases/latest) come with precompiled static binaries for Linux and macOS. Download one and put it somewhere in your shell's [`PATH`](https://en.wikipedia.org/wiki/PATH_(variable)).
</details>

<details>
<summary>Via Cargo</summary>

```bash
cargo install ethersync
```
</details>

### 2. Install an editor plugin

- [Neovim](https://github.com/ethersync/ethersync-vim)
- [VS Code](https://marketplace.visualstudio.com/items?itemName=ethersync.ethersync)
- [VS Codium](https://open-vsx.org/extension/ethersync/ethersync)
 
## 📖 Basic usage

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

## 🔗 Further resources

Learn more about Ethersync in [the documentation](https://ethersync.github.io/ethersync).

### Community projects

- @schrieveslaach's [Jetbrains plugin](https://github.com/ethersync/ethersync-jetbrains)
- @winniehell's [web editor](https://github.com/ethersync/ethersync-web)
- @sohalt's [Emacs plugin](https://github.com/sohalt/ethersync.el)

## 🔨 Contributing

If you're interested in building new editor plugins, read the specification for the [daemon-editor protocol](https://ethersync.github.io/ethersync/editor-plugin-dev-guide).

For more information about Ethersync's design, refer to the list of [decision records](docs/decisions/).

If you find bugs, please [open an issue](https://github.com/ethersync/ethersync/issues) on Github, or [open a discussion](https://github.com/ethersync/ethersync/discussions) to ask us anything!

## 💚 Funded by

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
