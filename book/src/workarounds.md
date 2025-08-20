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

## Opening binary files in editors converts them to UTF-8

This happens because most editors are not well-equipped for editing binary data directly.

To edit a binary file together, first convert to a hexdump like this (`-R never` is to disable color output):

    xxd -R never binary_file > binary_file.hex

Then, edit the `.hex` file collaboratively. Finally, convert back to a binary:

    xxd -r binary_file.hex > binary_file
