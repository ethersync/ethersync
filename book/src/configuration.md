<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Configuration

There are two ways to configure what an Ethersync daemon will do.

## Command line flags

You can provide the options on the command line, after `ethersync daemon`:

- `--peer` specifies that you want to try connect to a peer (you'll be prompted for an identifier).
- `--port <port for your daemon>` specifies which port the daemon should listen for incoming connections.

For security reasons, it is not possible to provide the (sensitive) peer identifier via a command line flag.

## Configuration files

If you keep starting Ethersync with the same options, you can also put the following options into a configuration file at `.ethersync/config`:

```ini
peer = <peer "id#secret" you want to try connecting to>
port = <port for your daemon>
```
