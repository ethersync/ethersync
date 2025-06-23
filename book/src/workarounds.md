<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Common pitfalls and workarounds

Some things about Ethersync are currently still a bit annoying. Let us show you how to work around them!

## Restarting the daemon requires restarting the editor

The editor plugins currently only try to connect to Ethersync when they first start. If you need to restart the daemon for any reason, you will also need to restart all open editors to reconnect.

## Editing a file with tools that don't have Ethersync support

We are [planning](https://github.com/ethersync/ethersync/pull/133) to support this in a smoother way, but currently it's recommended to:
- turn off the daemon
- make your edits
- start the daemon again.

It will then compare the ["last seen"](local-first.md) state with what you have on disk and synchronize your edits to other peers.
