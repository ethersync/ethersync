# SPDX-FileCopyrightText: 2024 MangoIV <contact@mangoiv.com>
#
# SPDX-License-Identifier: AGPL-3.0-or-later

{pkgs ? import <nixpkgs> {}, ...}: rec {
  ethersync = pkgs.callPackage ./ethersync.nix {};
  ethersync-static = pkgs.pkgsStatic.callPackage ./ethersync.nix {};
  nvim-ethersync = pkgs.callPackage ./nvim-ethersync.nix {};
  neovim-with-ethersync = pkgs.callPackage ./neovim-with-ethersync.nix {inherit ethersync nvim-ethersync;};
}
