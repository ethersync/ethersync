# SPDX-FileCopyrightText: 2024 MangoIV <contact@mangoiv.com>
#
# SPDX-License-Identifier: AGPL-3.0-or-later

(final: prev: let
  packages = import ./default.nix {pkgs = final;};
in {
  inherit (packages) ethersync neovim-with-ethersync;
  vimPlugins = prev.vimPlugins // {inherit (packages) nvim-ethersync;};
})
