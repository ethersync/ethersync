<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Common pitfalls and workarounds

Some things about Ethersync are currently still a bit annoying. Let us show you how to work around them!

## Restarting the daemon requires restarting the editor

The editor plugins currently only try to connect to Ethersync when they first start. If you need to restart the daemon for any reason, you will also need to restart all open editors to reconnect.

## Can't work on files from multiple projects in one editor session

The editor plugins currently only connect to a single daemon, when the first file from a shared directory is opened.
To work on files from another project, either use a second editor instance, or close the first one.
