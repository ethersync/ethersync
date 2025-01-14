<!--
SPDX-FileCopyrightText: 2024 Danny McClanahan <dmcC2@hypnicjerk.ai>
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

This directory contains integration tests for Ethersync. They assume that your `PATH` contains:

- An `nvim` with installed Ethersync plugin, and
- an `ethersync` binary (for connecting via `ethersync client`).

To run all integration tests, run:

```bash
cargo test
```

To run only a specific integration test, run:

```bash
cargo test --test <name>
```
