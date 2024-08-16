{naersk, ...}: (final: prev: rec {
  ethersync = final.callPackage ./ethersync.nix { naersk-lib = final.callPackage naersk {}; };
  vimPlugins = prev.vimPlugins // { 
    nvim-ethersync = final.callPackage ./nvim-ethersync.nix { };
  };
  neovim-with-ethersync = final.callPackage ./neovim-with-ethersync.nix {};
})
