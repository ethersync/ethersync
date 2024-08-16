{
  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs/nixos-unstable";
    parts.url = "github:hercules-ci/flake-parts";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };
  outputs = inputs: inputs.parts.lib.mkFlake {inherit inputs;} {
    systems = [ "x86_64-linux" "aarch64-linux" "x86_64-darwin" "aarch64-darwin" ];
    perSystem = {pkgs, ...}: {
      packages = rec {
        ethersync = pkgs.callPackage ./nix/ethersync.nix { naersk-lib = pkgs.callPackage inputs.naersk {}; };
        nvim-ethersync = pkgs.callPackage ./nix/nvim-ethersync.nix {};
        neovim-with-ethersync = pkgs.callPackage ./nix/neovim-with-ethersync.nix { inherit ethersync; vimPlugins = pkgs.vimPlugins // { inherit  nvim-ethersync; }; };
      };
      devShells.default = import ./nix/shell.nix { inherit pkgs; };
    };
    flake.overlays.default = import ./nix/overlay.nix { inherit (inputs) naersk; };
  };
}
