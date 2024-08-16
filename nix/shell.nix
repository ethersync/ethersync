{pkgs ? import <nixpkgs> {}, lib ? pkgs.lib, ...}: pkgs.mkShell {
  packages =
    (with pkgs; [cargo rustc neovim])
    ++ lib.optionals pkgs.hostPlatform.isDarwin [pkgs.darwin.apple_sdk.frameworks.CoreServices pkgs.libiconv];
}
