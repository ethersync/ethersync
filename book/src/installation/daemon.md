# Daemon

You might be able to use one of the following packages, or you could try a manual installation.

## Arch Linux

Install the [ethersync-git](https://aur.archlinux.org/packages/ethersync-git) package from the AUR.

## Nix

This repository provides a Nix flake. You can put it in your PATH like this:

```bash
nix shell github:ethersync/ethersync
```

If you want to install it permanently, you probably know what your favorite approach is.

> 💡 Tip:  
>
> You can use the Nix package on any Linux or MacOS system!

## Manual installation

You will need a [Rust](https://www.rust-lang.org) installation. You can then compile the daemon like this:

```bash
git clone git@github.com:ethersync/ethersync
cd ethersync/daemon
cargo build --release
```

This should download all dependencies, and successfully compile the project.

For the next steps to succeed you need to make sure that the resulting `ethersync` binary is in your shell PATH.
One option to do this temporarily is to run this command in the terminal:

```bash
export PATH="$PWD/target/release:$PATH"
```

## Confirm the installation

To confirm that the installation worked, try running:

```bash
ethersync
```

This should show the available options.
