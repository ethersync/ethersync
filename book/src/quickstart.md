# Getting started

To collaborate on a directory called `playground`, follow these steps:


## 1. Create an "Ethersync-enabled" directory

Our current convention is to have a subdirectory called `.ethersync` in an Ethersync-enabled directory. Create it like this:

```bash
mkdir -p playground/.ethersync
cd playground
touch file
```

## 2. Start the daemon

In a group, one person needs to start the session, and the others connect to it.

- As the **starting peer**, run:

    ```bash
    ethersync daemon
    ```

    This will print a connection address (like `/ip4/192.168.23.42/tcp/4242/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv`) which others in the same local network can use to connect to you. (See the FAQ below on how to connect from another local network.)

- As a **joining peer**, specify the address of the starting peer:

    ```bash
    ethersync daemon --peer /ip4/192.168.23.42/tcp/4242/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv
    ```

## 3. Start collaborating in real-time!

You can now open, edit, create and delete files in the shared directory, and connected peers will get your changes! For example, open a new file:

```bash
nvim file
```

If everything went correctly, you should see `Ethersync activated!` in Neovim's messages and `Client connection established.` in the logs of the daemon.

> [!TIP]
> If that doesn't work, make sure that there's an `.ethersync` directory in the `playground`, and that the `ethersync` command is in the PATH in the terminal where you run Neovim.
