<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Neovim plugin for 🍃 [Ethersync](https://github.com/ethersync/ethersync)

> ![IMPORTANT]
>
> This plugin requires at least Neovim 0.7.0.

## Installation

The Neovim plugin is located at <https://github.com/ethersync/ethersync-vim>, so you would usually add the string `"ethersync/ethersync-vim"` to your plugin manager. Here's some example configuration blocks:

### Lazy

```lua
{
  "ethersync/ethersync-vim",
  keys = { { "<leader>j", "<cmd>EthersyncJumpToCursor<cr>" } },
  lazy = false,
}
```

### pckr.nvim

```lua
{
  "ethersync/ethersync-vim",
  config = function()
    vim.keymap.set('n', '<leader>j', '<cmd>EthersyncJumpToCursor<cr>')
  end
}
```

### Manual installation

If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

```
git clone git@github.com:ethersync/ethersync-vim $HOME/.local/share/nvim/site/pack/plugins/start/ethersync
```

### Nix

For testing purposes, you can run an Ethersync-enabled Neovim like this:

```bash
nix run github:ethersync/ethersync#neovim
```

## Confirm the installation

To confirm that the plugin is installed, try running the `:EthersyncInfo` command in Neovim. It should show the message "Not connected to Ethersync daemon."

## Tips

We recommend creating a mapping for the `:EthersyncJumpToCursor` command (for example, `<Leader>j`, which jumps to another user's cursor.
