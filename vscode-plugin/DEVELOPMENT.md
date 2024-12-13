# VS Code Ethersync plugin

## How to run locally

1. `npm install`
2. Open this directory in VS Code, then start debugging with F5.
3. Open a file in an Ethersync-enabled directory to launch the plugin.

## How to release on the Visual Studio Marketplace and Open VSX

1. Bump the version in `package.json`.
2. Run `vsce publish`.
3. Run `npx ovsx publish --pat <token>` (or provide the token via `OVSX_PAT`).
