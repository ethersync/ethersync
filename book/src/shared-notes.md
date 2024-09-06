# Using Ethersync for writing shared notes

Another use case for Ethersync is to have a **long-lasting collaboration session** on a directory of text files (over the span of months or years). This is similar to how you would use Google Docs, Ethersync or Hedgedoc to work on text. It would be suited for groups who want to write notes or documentation together.

This use case is different from the "pair-programming" use case, because there, all peers are online at the same time. When you're working on a directory of notes for a longer time, it might happen that you make a change to a file, and then go offline, while the other peers are also offline. Still, you want other peers to be able to receive your changes.

We suggest to use a ["cloud peer"](connection-making.md#cloud-peer), a peer that is always online.

## Step-by-step guide

You need to have access to a server on the Internet, and install the Ethersync daemon there.

### 1. Set up the directory

On the server, create a new directory for your shared project, as well as an `.ethersync` directory inside it:

```bash
mkdir my-project/.ethersync
cd my-project
```

### 2. Configure the daemon

You'll want to use a stable secret passphrase and a stable port on the server, so put those into the configuration file:

```bash
echo "secret=your-passphrase-here" >> .ethersync/config
echo "port=4242" >> .ethersync/config
```

### 3. Start the daemon

Launch the daemon in a way where it will keep running once you disconnect from your terminal session on the server. You could use `screen`, `tmux`, write a systemd service, or, in the easiest case, launch it with `nohup`:

```bash
nohup ethersync daemon &
```

### 4. Collaborate!

Other peers can now connect to the "cloud peer". It is most convenient for them to also use a configuration file like this:

```bash
echo "secret=your-passphrase-here" >> .ethersync/config
echo "peer=/ip4/<server ip>/tcp/<port>/p2p/<peerid>" >> .ethersync/config
```

Then, they can connect anytime using

```bash
ethersync daemon
```
