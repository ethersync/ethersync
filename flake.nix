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
      vimPlugins =
        prev.vimPlugins
        // {
          nvim-ethersync = final.vimUtils.buildVimPlugin {
            name = "ethersync";
            src = ./vim-plugin;
          };
        };
      neovim-with-ethersync = let
        nvim-custom = prev.neovim.override {
          configure = {
            packages.plugins = {
              start = [
                vimPlugins.nvim-ethersync
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
        nvim-ethersync
        neovim-with-ethersync
        ;
      default = ethersync;
    });
    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        packages =
          (with pkgs; [cargo rustc neovim])
          ++ (
            # macOS systems seem to require these extra packages for building Rust code.
            if (lib.strings.hasInfix "darwin" pkgs.system)
            then (with pkgs; [darwin.apple_sdk.frameworks.CoreServices libiconv])
            else []
          );
      };
    });
  };
}
