# SPDX-FileCopyrightText: 2024 MangoIV <contact@mangoiv.com>
#
# SPDX-License-Identifier: AGPL-3.0-or-later

{
  pkgs ? import <nixpkgs> {},
  lib ? pkgs.lib,
  ...
}:
pkgs.mkShell {
  packages =
    (with pkgs; [cargo rustc neovim alejandra])
    ++ lib.optionals pkgs.hostPlatform.isDarwin [
      pkgs.darwin.apple_sdk.frameworks.CoreServices
      pkgs.darwin.apple_sdk.frameworks.SystemConfiguration
      pkgs.libiconv
    ];
}
