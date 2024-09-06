{
  neovim,
  writeShellScriptBin,
  lib,
  ethersync,
  nvim-ethersync,
  ...
}: let
  nvim-custom = neovim.override {
    configure = {
      packages.plugins.start = [nvim-ethersync];

      # In Nix' standard environment, we can't write to $HOME, so we need to
      # disable swapfiles, and LSP log files.
      customRC = ''
        set noswapfile
        lua vim.lsp.set_log_level("off")
      '';
    };
  };
in
  writeShellScriptBin "nvim" ''
    PATH=${lib.makeBinPath [ethersync]}:$PATH ${nvim-custom}/bin/nvim $@
  ''
