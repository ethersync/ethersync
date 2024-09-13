# Neovim Plugin

> ❗ **Important**
>
> The plugin currently requires Neovim v0.10.

Again, we have several options of how to install the Neovim plugin:

## Lazy

If you're using the [Lazy](https://github.com/folke/lazy.nvim) plugin manager, you can use a configuration block like this:

```lua
{
  "ethersync/ethersync",
  config = function(plugin)
      -- Load the plugin from a subfolder:
      vim.opt.rtp:append(plugin.dir .. "/vim-plugin")
      require("lazy.core.loader").packadd(plugin.dir .. "/vim-plugin")
  end,
  keys = { { "<leader>j", "<cmd>EthersyncJumpToCursor<cr>" } },
  lazy = false,
}
```

## Nix

For testing purposes, you can run an Ethersync-enabled Neovim like this:

```bash
nix run github:ethersync/ethersync#neovim
```

## Manual installation

If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

Clone the Ethersync repository:

```bash
git clone git@github.com:ethersync/ethersync
```

Link to the plugin directory from nvim:

```bash
mkdir -p $HOME/.local/share/nvim/site/pack/plugins/start
cd ethersync # make sure you're in the root of the project
ln -s $PWD/vim-plugin $HOME/.local/share/nvim/site/pack/plugins/start/ethersync
```

## Confirm the installation

To confirm that the plugin is installed, try running the `:EthersyncInfo` command in Neovim. It should show a message like "Not connected to the Ethersync daemon".
