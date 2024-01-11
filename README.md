# Ethersync

Ethersync enables real-time co-editing of local text files. You will be able to use it for pair programming or note-taking, for example.

Currently, we have a **simple working prototype**, but we're still at the beginning of development.
Thus be warned, that everything is in flux and can change/break/move around quickly.

## Components

Ethersync consists of three components:

- A central **server** is responsible for connecting people.
- Every participant needs a **daemon**, that runs on their local machine, and connects directories to the server.
- **Editor plugins** connect to the daemon, send it what you type, and receive other peoples' changes.
Currently, there's a plugin Neovim, but other editor integrations are planned.

## Setup

### Server

- find the source in [server](./server) TODO

Install the dependencies...

```bash
cd etherwiki
npm install
```

...then start [Rollup](https://rollupjs.org):

```bash
npm run dev
```

Navigate to [localhost:5000](http://localhost:5000).
This UI can be used for development/debugging purposes, but isn't needed.
The important part is, that this provides a Websocket Server, which has to be configured in the daemon.

### Daemon

- the source is in [daemon](./daemon)

After the usual `npm install`, to start up the daemon with a directory (say, `output`) you need a config file in `.ethersync/config`:
```
cd daemon
npm install
mkdir -p output/.ethersync # the output directory contains the locally synced files and a config
echo "etherwiki=http://localhost:5000#playground" > output/.ethersync/config # where localhost is the server instance from above
```

To run it, start with:
```
npm run ethersync --directory=output
```

### Neovim Plugin

Install the [plugin](./vim-plugin) using your favorite plugin manager. For example, with Lazy, this is the configuration:

```lua
{
    dir = path/to/ethersync/vim-plugin",
}
```

## Sponsors

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/).

## License

TBD
