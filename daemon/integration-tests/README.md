This directory contains integration tests for Ethersync. They assume that your `PATH` contains:

- An `nvim` with installed Ethersync plugin, and
- an `ethersync` binary (for connecting via `ethersync client`).

To run all integration tests, run:

```bash
ethersync test
```

To run only a specific integration test, run:

```bash
ethersync test --test <name>
```
