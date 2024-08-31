# Common pitfalls and workarounds

Some things about Ethersync are currently still a bit annoying. Let us show you how to work around them!

## Sharing multiple projects

Ethersync currently only supports sharing a single project directory per daemon. If you want to sync more than one project, you can do so by starting a second daemon. The trick is to use a different socket for the editors to connect to.

1. When staring the second daemon, use the `--socket-path` option, like this:

    ```bash
    ethersync daemon --socket-path /tmp/ethersync2
    ```

2. Before opening a file in the second project directory, set the `ETHERSYNC_SOCKET` environment variable to the correct path, like this:

    ```bash
    export ETHERSYNC_SOCKET=/tmp/ethersync2
    ```

## Restarting the daemon

The editor plugins currently only try to connect to Ethersync when they first start. If you need to restart the daemon for any reason, you will also need to restart all open editors to reconnect.
