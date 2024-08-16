(final: prev: let 
  packages = import ./default.nix { pkgs = final; }; 
in {
  inherit (packages) ethersync neovim-with-ethersync;
  vimPlugins = prev.vimPlugins // { inherit (packages) nvim-ethersync; };
})
