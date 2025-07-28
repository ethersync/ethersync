<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# File ownership

Ethersync synchronizes edits immediately to each peer, and the receiving daemon will write changes to disk immediately. This means that the concept of "saving a file" does not exist anymore when using Ethersync - everything is auto-saved.

Sometimes the peer has the file already open in an editor, sometimes not. We are using a concept called "ownership" to define whether changes by external tools to the file are taken into account. Either the daemon or the editor can have it.

## Daemon has ownership

The daemon has ownership of a file if it *is not open in an editor on that computer*. In this case the daemon will pick up changes done by external tools.

## Editor has ownership

By opening a file in an editor with Ethersync plugin, that editor takes "ownership" of the file – the daemon will not pick up external changes anymore. Instead, the editor's buffer content is seen as the "truth".
