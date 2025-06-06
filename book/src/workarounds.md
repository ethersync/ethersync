<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Common pitfalls and workarounds

Some things about Ethersync are currently still a bit annoying. Let us show you how to work around them!

## Restarting the daemon requires restarting the editor

The editor plugins currently only try to connect to Ethersync when they first start. If you need to restart the daemon for any reason, you will also need to restart all open editors to reconnect.
