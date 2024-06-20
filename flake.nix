{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    nixpkgs,
    naersk,
    ...
  }: let
    forAllSystems = function:
      nixpkgs.lib.genAttrs [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ] (system: function nixpkgs.legacyPackages.${system});
    neovim-with-ethersync-plugin = pkgs:
      pkgs.neovim.override
      {
        configure = {
          packages.plugins = {
            start = [
              (pkgs.vimUtils.buildVimPlugin {
                name = "ethersync";
                src = ./vim-plugin;
              })
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
    ethersync = pkgs:
      (pkgs.callPackage naersk {}).buildPackage {
        src = ./daemon;
      };
  in {
    packages = forAllSystems (pkgs: {
      default = ethersync pkgs;
      neovim = neovim-with-ethersync-plugin pkgs;
    });

    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        nativeBuildInputs = [pkgs.cargo pkgs.rustc (ethersync pkgs) (neovim-with-ethersync-plugin pkgs)];
      };
    });
  };
}
