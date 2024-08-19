# 🇳 Neovim Plugin

You will also need an **editor plugin** connect to the daemon, send it what you type, and receive other peoples' changes.
Right now, we are offering a Neovim plugin. More plugins are planned.

> [!IMPORTANT]
> The plugin currently requires Neovim v0.10.

Again, we have several options of how to install it:

<details>
  <summary>Lazy plugin manager</summary>
  <br>

  If you're using [Lazy](https://github.com/folke/lazy.nvim), you can use a configuration like this:

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
</details>

<details>
  <summary>Nix</summary>
  <br>

  For testing purposes, you can run an Ethersync-enabled Neovim like this:

  ```bash
  nix run github:ethersync/ethersync#neovim
  ```
</details>

<details>
  <summary>Manual installation</summary>
  <br>

  If you're not using a plugin manager, here's a "quick and dirty" way to install the plugin:

  If you don't already have the repo (i.e you choose a packaged option above):
  ```bash
  git clone git@github.com:ethersync/ethersync
  ```

  Link to the plugin directory from nvim:
  ```bash
  mkdir -p $HOME/.local/share/nvim/site/pack/plugins/start
  cd ethersync # make sure you're in the root of the project
  ln -s $PWD/vim-plugin $HOME/.local/share/nvim/site/pack/plugins/start/ethersync
  ```
</details>

To confirm that the plugin is installed, try running the `:EthersyncInfo` command in Neovim.
