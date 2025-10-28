<!--
SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Development of the Teamtype plugin for Neovim

## Installing a local version

Plugin managers usually have a way to install a plugin from a local directory.
For example, this is a suitable configuration block for Lazy:

```lua
    return {
        dir = os.getenv("HOME") .. "/path/to/teamtype/nvim-plugin",
        keys = { { "<leader>j", "<cmd>TeamtypeJumpToCursor<cr>" } },
        lazy = false,
    }
```

## Deployment

The plugin will automatically be mirrored to the `develop` branch of https://github.com/teamtype/teamtype-nvim.

Upon release, it will be published on the `main` branch.
