<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Neovim plugin for 🍃 [Teamtype](https://github.com/teamtype/teamtype)-compatible collaborative software

This plugin adds real-time collaborative editing functionality to Neovim.
You can use it for pair programming or note-taking, for example. It is mainly
meant to be used with Teamtype, but can also be configured to work with other
collaborative software speaking the same protocol.

> [!IMPORTANT]
>
> This plugin requires at least Neovim 0.7.0 (which was released in 2022).

## Installation

### Manual installation

If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

```
git clone https://github.com/teamtype/teamtype-nvim $HOME/.local/share/nvim/site/pack/plugins/start/teamtype
```

### Plugin managers

Usually, you will add the string `"teamtype/teamtype-nvim"` to your plugin manager. Here's some example configuration blocks:

#### Lazy

```lua
{
  "teamtype/teamtype-nvim",
  keys = { 
    { "<leader>ej", "<cmd>TeamtypeJumpToCursor<cr>" },
    { "<leader>ef", "<cmd>TeamtypeFollow<cr>" },
  },
  lazy = false,
}
```

#### pckr.nvim

```lua
{
  "teamtype/teamtype-nvim",
  config = function()
    vim.keymap.set('n', '<leader>ej', '<cmd>TeamtypeJumpToCursor<cr>')
    vim.keymap.set('n', '<leader>ef', '<cmd>TeamtypeFollow<cr>')
  end
}
```

### Nix

For testing purposes, you can run an Teamtype-enabled Neovim like this:

```bash
nix run github:teamtype/teamtype#neovim
```

## Confirm the installation

To confirm that the plugin is installed, try running the `:TeamtypeInfo` command in Neovim. It should show the message "Not connected to Teamtype daemon."

## Tips

We recommend creating mappings for the `:TeamtypeJumpToCursor` and `:TeamtypeFollow` command, see above configurations for examples.

## Configuration

See the [help file](doc/teamtype.txt) for details on configuring this plugin.
