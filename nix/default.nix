{pkgs ? import <nixpkgs> {}, ...}: rec {
  ethersync = pkgs.callPackage ./ethersync.nix { };
  nvim-ethersync = pkgs.callPackage ./nvim-ethersync.nix { };
  neovim-with-ethersync = pkgs.callPackage ./neovim-with-ethersync.nix { inherit ethersync nvim-ethersync; };
}
