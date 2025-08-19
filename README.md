<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# 🍃 Ethersync

*Multiplayer mode for your text editor!*

Ethersync enables real-time collaborative editing of local text files. You can use it for pair programming or note-taking, for example. It's the missing real-time complement to Git!

![Demo video for how to make a connection and of collaborating in Neovim](https://files.blinry.org/ethersync-share-join-demo.gif)

## Features

- 👥 Edit files at the same time, across different text editors
- 📍 See your peers' cursors and selections
- 🗃️ Work on entire projects, the way you're used to
- 🔒 Encrypted peer-to-peer connections, no need for a server
- ✒️ Local-first: You always have full access, even offline
- 🧩 [Simple JSON-RPC protocol](https://ethersync.github.io/ethersync/editor-plugin-dev-guide.html) for writing new editor plugins

## What Ethersync is not

We are not a company, and don't sell anything. We don't require you to create an account. We don't have access to your data, and don't use it to train AI algorithms. We don't serve you ads, and don't track you.

We're just a bunch of people building something we want to see in the world.

## 🚦 Project status

The project is under active development right now. We use it every day, but there's still some [bugs](https://github.com/ethersync/ethersync/issues?q=sort%3Aupdated-desc+is%3Aissue+is%3Aopen+%28label%3Abug+OR+type%3ABug%29) to be aware of.

## 📥 Installation

### 1. Install the `ethersync` command

Ethersync works on Linux, macOS, Android, and on the Windows Subsystem for Linux.

<details>
<summary>Binary releases</summary>

The [releases on GitHub](https://github.com/ethersync/ethersync/releases/latest) come with precompiled static binaries. Download one and put it somewhere in your shell's [`PATH`](https://en.wikipedia.org/wiki/PATH_(variable)):

- `x86_64-unknown-linux-musl` for Linux
- `universal-apple-darwin` for macOS
- `aarch64-unknown-linux-musl` for Android (use a terminal emulator like [Termux](https://termux.dev))

</details>

<details>
<summary>Arch Linux</summary>

```
sudo pacman -S ethersync
```
</details>

<details>
<summary>Nix</summary>

To put `ethersync` in your PATH temporarily, run:

```
nix shell nixpkgs#ethersync
```

Make sure to also have it in your PATH when you run the editors, or install it to your environment in your preferred way.
</details>

<details>
<summary>Via Cargo</summary>

```bash
cargo install ethersync
```
</details>

### 2. Install an editor plugin

- [Neovim](https://github.com/ethersync/ethersync-nvim)
- VS Code/Codium: Install the "Ethersync" extension from the marketplace

## 📖 Basic usage

In the directory you want to share:

```
$ ethersync share

    To connect to you, another person can run:

    ethersync join 5-hamburger-endorse

Peer connected: adfa90edd932732ddf242f24dc2dcd6156779e69966d432fcb3b9fe3ae9831ab
```

Another person, in a separate directory (also works on the same computer):

```
$ ethersync join 5-hamburger-endorse

Derived peer from join code. Storing in config (overwriting previous config).
Storing peer's address in .ethersync/config.
Connected to peer: 5e6b787fff79074735eb9b56939269100de1e37bc7f7a4d29c277cc24f7ee53d
```

The directories are now connected, and changes will be synced instantly. You can open text files (using editors with an Ethersync plugin), and start collaborating in real time! :sparkles:

## 🎓 Learn more

- Learn more about Ethersync in [the documentation](https://ethersync.github.io/ethersync).
- Watch a [10-minute talk](https://fosdem.org/2025/schedule/event/fosdem-2025-4890-ethersync-real-time-collaboration-in-your-text-editor-/) given at FOSDEM 2025.
- Watch a (German) [1-hour talk](https://media.ccc.de/v/2024-355-ethersync-echtzeit-kollaboration-in-deinem-texteditor-) given at MRMCD 2024.

## 🚧 Community projects

(These are all work-in-progress!)

- @schrieveslaach's [Jetbrains plugin](https://github.com/ethersync/ethersync-jetbrains)
- @sohalt's [Emacs plugin](https://github.com/sohalt/ethersync.el)
- @winniehell's [web editor](https://github.com/ethersync/ethersync-web)

## 🔨 Contributing

We'd love to receive your patches and other contributions! Small patches are very welcome as PRs. Before starting to implement a new big feature, please briefly [check in with us](#contact) so we can discuss how it fits in with our ideas for the project.

If you're interested in building new editor plugins, read the [editor plugin development guide](https://ethersync.github.io/ethersync/editor-plugin-dev-guide).
For more information about Ethersync's design, refer to the list of [decision records](docs/decisions/).

If you find bugs, please [open an issue](https://github.com/ethersync/ethersync/issues) on Github!

## ☎️ Contact

Feel free to [open a discussion on Github](https://github.com/ethersync/ethersync/discussions) to ask us anything! Other good channels:

- Mastodon: [@ethersync@fosstodon.org](https://fosstodon.org/@ethersync)
- Email: <span>e<span title="ihate@spam.com&lt;/span&gt;">t</span>hersync</span>@zormit<i title="&lt;/i&gt;mailto:">.</i>de

## 💚 Thanks

Ethersync received funding from [NLNet](https://nlnet.nl)'s [NGI0 Core Fund](https://nlnet.nl/core/) throughout 2024.

Thanks to the [Prototype Fund](https://www.prototypefund.de/) and the [Federal Ministry of Research, Technology and Space](https://www.bmbf.de/EN/) for funding this project in 2025.

<a href="https://nlnet.nl/"><img src="https://upload.wikimedia.org/wikipedia/en/a/a4/NLnet_Foundation_logo.svg" alt="Logo of the NLnet Foundation" style="height: 70px;"></a>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="https://prototypefund.de/en/"><img src="https://upload.wikimedia.org/wikipedia/commons/b/ba/Prototype_Fund_Logo_2025.svg" alt="Logo of the Prototype Fund" style="height: 70px;"></a>
&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;&nbsp;
<a href="https://okfn.de/en/"><img src="https://upload.wikimedia.org/wikipedia/commons/4/4d/Open_Knowledge_Foundation_Deutschland_Logo.svg" alt="Logo of the Open Knowledge Foundation Germany" style="height: 100px;"></a>
&nbsp;&nbsp;
<a href="https://www.bmbf.de/EN/"><img src="https://upload.wikimedia.org/wikipedia/commons/d/df/BMFTR_Logo.svg" alt="Logo of the German Federal Ministry of Research, Technology and Space" style="height: 110px;"></a>

Ethersync is based on [Automerge](https://automerge.org), [Iroh](https://www.iroh.computer), and [Magic Wormhole](https://magic-wormhole.readthedocs.io).

And finally, thanks to everyone who helped us beta-test, or reported issues!

## 📜 License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

This project is [REUSE](https://reuse.software) compliant, see the headers of each file for licensing information.
