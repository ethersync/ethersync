<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# VS Code plugin for 🍃 [Ethersync](https://github.com/ethersync/ethersync)-compatible projects

Note: You will need to install and run a "collaboration server" like Ethersync in addition to this plugin!

Refer to the [main project](https://github.com/ethersync/ethersync) for documentation and usage instructions.

## Configuration

A configuration for connecting to Ethersync daemons is provided as a default value.

You can also use this plugin with other collaboration servers.

To add other configurations, add a section like this to your `settings.json` (you can access it using *F1 -> Preferences: Open User Settings (JSON)*):

```jsonc
"ethersync.configs": {
  /* Default configuration */
  "ethersync": {
    "cmd": [ "ethersync", "client" ],
    "rootMarkers": [ ".ethersync" ]
   },

  /* Hypothetical configuration for another program */
  "http": {
    "cmd": [ "ethersync-http" ],
    "rootMarkers": [ ".ethersync-http" ]
  },
}
```

Available options:

- `cmd`: Array of strings to specify the command to be launched to connect to the collaboration server.
- `rootMarkers`: Array of strings to indicate if a directory should be considered as a collaboration project for this configuration.
