{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    naersk,
    ...
  }: let
    supportedSystems = [
      "x86_64-linux"
      "aarch64-linux"
      "x86_64-darwin"
      "aarch64-darwin"
    ];
    lib = nixpkgs.lib;
    forAllSystems = function:
      lib.genAttrs supportedSystems (system:
        function (import nixpkgs {
          inherit system;
          overlays = [self.overlays.default];
        }));
  in {
    overlays.default = final: prev: rec {
      ethersync = (final.callPackage naersk {}).buildPackage {
        src = ./daemon;
      };
      neovim-plugin = final.vimUtils.buildVimPlugin {
        name = "ethersync";
        src = ./vim-plugin;
      };
      neovim = let
        nvim-custom = prev.neovim.override {
          configure = {
            packages.plugins = {
              start = [
                neovim-plugin
              ];
            };
            # In Nix' standard environment, we can't write to $HOME, so we need to
            # disable swapfiles, and LSP log files.
            customRC = ''
              set noswapfile
              lua vim.lsp.set_log_level("off")
            '';
          };
        };
      in
        final.writeShellScriptBin "nvim" ''
          PATH=${lib.makeBinPath [ethersync]}:$PATH ${nvim-custom}/bin/nvim $@'';
    };
    packages = forAllSystems (pkgs: rec {
      inherit
        (pkgs)
        ethersync
        neovim-plugin
        neovim
        ;
      default = ethersync;
    });

    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages = with pkgs; [cargo rustc neovim];
      };
    });
  };
}
