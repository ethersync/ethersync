# Using Ethersync for pair programming

One use case for Ethersync is to do a **short collaboration session** on an existing project. You could do this, for example, to pair program with another person, editing code together at the same time.

This would also work for more than two people. One person will start the session, and others can connect to it.

## Step-by-step guide

### Starting conditions

If both people already have a copy of the project, make sure you're on the same state – for example, by making sure that you're on the same commit with a clean working tree. If the joining peer has a different state, those changes will be overwritten.

An alternative is that the joining peer starts from scratch, with an empty directory.

Make sure you're both inside the project directory on the command line.

### Create the `.ethersync` directory

This is our convention to mark a project as shareable: It needs to have a directory called `.ethersync` in it. So both peers should make sure that it exists:

```bash
mkdir .ethersync
```
Note that this directory, similar to a `.git` directory will *not* be synchronized.

### First peer

To start the session, run:

```bash
ethersync daemon
```

This will print, among other initialization information, two things you need to tell the other peers:

- A connection address like `/ip4/192.168.23.42/tcp/58063/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv`. This is what libp2p calls a [multiaddress](https://docs.libp2p.io/concepts/fundamentals/addressing/) – it contains your IP address, the TCP port, and a "peer ID" (which is used by connecting peers to make sure that they're actually connecting to the correct peer, and not to a "man in the middle").
- A secret passphrase, that is randomly generated each time you start the daemon. If you want to use a stable secret, we recommend putting it into the [configuration file](../features/configuration.md).

### Other peers

To join a session, run:

```bash
ethersync demon --peer <multiaddr> --secret <secret>
```

This should show you a message like "Connected to peer ...". The hosting daemon should show a message like "Peer connected".

If you prefer, it's also possible to use the [configuration file](../features/configuration.md) to provide multiaddress and secret.

### Collaborate!

Connected peers can now open files and edit them together. Note the current restrictions on [file events](../features/file-events.md) and the [common pitfalls](../features/workarounds.md).

### Stop Ethersync

To stop collaborating, stop the daemon (by pressing Ctrl-C in its terminal). Both peers will still have the code they worked on, and can continue their work independently.

### Reconnect later

If you later want to do another pairing session, make sure that you understand Ethersync's [offline support](../features/offline-support.md) feature. When you re-start Ethersync, it will scan for changes you've made in the meantime, and try to send them to the other peer. It is probably safest if you delete the CRDT state in `.ethersync/doc` as a joining peer. The hosting peer doesn't need to do that, it will simply update their state to the latest file content and share that with others.

## How to connect across different local networks?

For two people in the same network, the connection will just work. If you want to connect to someone in another local network, you'll currently need to do a workaround:

You need to enable port forwarding on your router. Specifically, the hosting peer needs to configure their router in such a way that it forwards incoming connections on the port you're using with Ethersync (you can specify a fixed port with the `--port` option) to their local machine.
