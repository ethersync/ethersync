# Daemon

You might be able to use one of the following packages:

## Arch Linux

Install the [ethersync-bin](https://aur.archlinux.org/packages/ethersync-bin) package from the AUR.

## Nix

> 💡 **Tip**
>
> You can use the Nix package on any Linux or MacOS system!

This repository provides a Nix flake. You can put it in your `PATH` like this:

```bash
nix shell github:ethersync/ethersync
```

If you want to install it permanently, you probably know what your favorite approach is.

## Binary releases

The releases on GitHub come with [precompiled static binaries](https://github.com/ethersync/ethersync/releases/latest) for Linux and macOS. Download one and put it somewhere in your shell's [`PATH`](https://en.wikipedia.org/wiki/PATH_(variable)), so that you can run it with `ethersync`.

## Via Cargo

If you have a [Rust](https://www.rust-lang.org) installation, you can install Ethersync with `cargo`:

```bash
cargo install ethersync
```

## Confirm the installation

To confirm that the installation worked, try running:

```bash
ethersync
```

This should show the available options.
