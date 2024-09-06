{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    parts.url = "github:hercules-ci/flake-parts";
  };
  outputs = inputs:
    inputs.parts.lib.mkFlake {inherit inputs;} {
      systems = ["x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin"];
      perSystem = {pkgs, ...}: let
        ethersync-packages = import ./nix/default.nix {inherit pkgs;};
      in {
        packages = rec {
          inherit (ethersync-packages) ethersync ethersync-static nvim-ethersync;
          default = ethersync;
          neovim = ethersync-packages.neovim-with-ethersync;
        };
        devShells.default = import ./nix/shell.nix {inherit pkgs;};
      };
      flake.overlays.default = import ./nix/overlay.nix {};
    };
}
