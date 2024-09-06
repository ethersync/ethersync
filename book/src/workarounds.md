# Common pitfalls and workarounds

Some things about Ethersync are currently still a bit annoying. Let us show you how to work around them!

## Sharing multiple projects requires configuring the socket

Ethersync currently only supports sharing a single project directory per daemon. If you want to sync more than one project, you can do so by starting a second daemon. The trick is to use a different socket for the editors to connect to.

1. When starting the second daemon, use the `--socket-path` option, like this:

    ```bash
    ethersync daemon --socket-path /tmp/ethersync2
    ```

2. Before opening a file in the second project directory, set the `ETHERSYNC_SOCKET` environment variable to the correct path, like this:

    ```bash
    export ETHERSYNC_SOCKET=/tmp/ethersync2
    ```

## Restarting the daemon requires restarting the editor

The editor plugins currently only try to connect to Ethersync when they first start. If you need to restart the daemon for any reason, you will also need to restart all open editors to reconnect.

## Editing a file with tools that don't have Ethersync support

We are [planning](https://github.com/ethersync/ethersync/pull/133) to support this in a smoother way, but currently it's recommended to:
- turn off the daemon
- make your edits
- start the daemon again.

It will then compare the ["last seen"](local-first.md) state with what you have on disk and synchronize your edits to other peers.
