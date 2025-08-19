<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# VS Code Ethersync plugin

## How to run locally

1. `npm install`
2. Open this directory in VS Code, then start debugging with F5.
3. Open a file in an Ethersync-enabled directory to launch the plugin.

## How to run locally without "host"

1. `npm install`
2. `npm run watch`
3. Within VS Code, run "Developer: Install Extension from Location...", then select this directory.

## How to release on the Visual Studio Marketplace and Open VSX

1. Bump the version in `package.json` and `package-lock.json`
    - Use an odd minor number (e.g. 0.3.0) for a pre-release version.
2. Run `vsce publish`.
    - Use `--pre-release` for a pre-release version.
3. Run `npx ovsx publish --pat <token>` (or provide the token via `OVSX_PAT`).
    - Use `--pre-release` for a pre-release version.
