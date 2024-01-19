# Ethersync

Ethersync enables real-time co-editing of local text files. You will be able to use it for pair programming or note-taking, for example.

Currently, we have a **simple working prototype**, but we're still at the beginning of development.
Thus be warned, that everything is in flux and can change/break/move around quickly.

## Components

Ethersync consists of three components:

- A central **server** is responsible for connecting people.
- Every participant needs a **daemon**, that runs on their local machine, and connects directories to the server.
- **Editor plugins** connect to the daemon, send it what you type, and receive other peoples' changes.
Currently, there's a plugin for Neovim, but other editor integrations are planned.

## Setup

We need to set up these three components. First, clone this repository:

```bash
git clone git@github.com:ethersync/ethersync
cd ethersync
```

### Server

First, let's start a local server instance:

```bash
cd server
npm install
npm run dev
```

The server also works without the other components, and offers a browser UI. You can try it by navigating to <http://localhost:5000). Create a wiki, and some pages.

### Daemon

But we also allow you to edit the content from your own text editor. For that, we need to connect the wiki to a local directory, using a daemon.

Let's say we want to connect a wiki running at <http://localhost:5000#playground> to the directory `playground`. Here's how you would configure it.

1. Create the directory, which will create the locally synced files and a configuration file:

        mkdir -p playground/.ethersync

2. Create the configuration file:

        echo "etherwiki=http://localhost:5000#playground" > playground/.ethersync/config

After that, let's start the daemon:

```
cd daemon
npm install
npm run ethersync --directory=path/to/playground
```

### Neovim Plugin

Finally, we need an editor plugin, for you to be able to edit the files in real time.

Install the [plugin](./vim-plugin) using your favorite plugin manager. For now, use the path to the `vim-plugin` directory in this repository. Consult the documentation of your plugin manager on how to do that. Example configuration for [Lazy](https://github.com/folke/lazy.nvim):

```lua
{
    dir = os.getenv("HOME") .. "/path/to/ethersync/vim-plugin",
}
```

## Usage

- Right now, you can only edit files which exist on the server
    - You might want to add one via the web interface :)
- When you edit a file in the output directory:
    - If it's installed correctly, you'll get an "Ethersync activated!" greeting.
    - Everything you edit is kept in sync.

## Sponsors

Thanks to [NLNet](https://nlnet.nl) for funding this project through the [NGI0 Core Fund](https://nlnet.nl/core/).

## License

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
