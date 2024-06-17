{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs = {nixpkgs, ...}: let
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
      pkgs.rustPlatform.buildRustPackage {
        pname = "ethersync";
        version = "0.2.0";
        src = ./daemon;
        cargoSha256 = "sha256-9ZRMcVwKzCmumake+s8Cy+lm5eMPFrr9n912/Z1nBAk=";
      };
  in {
    packages = forAllSystems (pkgs: {
      default = ethersync pkgs;
    });

    devShells = forAllSystems (pkgs: {
      default = pkgs.mkShell {
        nativeBuildInputs = [pkgs.cargo pkgs.rustc (neovim-with-ethersync-plugin pkgs) (ethersync pkgs)];
      };
    });

    # TODO: Running these checks seems broken.
    checks = forAllSystems (pkgs: {
      fuzzer = pkgs.stdenv.mkDerivation {
        name = "ethersync-fuzzer";
        nativeBuildInputs = [(ethersync pkgs) (neovim-with-ethersync-plugin pkgs)];
        src = ./.;
        doCheck = true;
        checkPhase = ''
          fuzzer
        '';
      };
    });
  };
}
