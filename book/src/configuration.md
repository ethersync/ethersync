# Configuration

There are two ways to configure what an Ethersync daemon will do.

## Command line flags

You can provide the options on the command line, after `ethersync daemon`:

- `--peer <multiaddr>` specifies which peer you want to try connect to.
- `--secret <passphrase>` specifies the shared secret passphrase. Peers must use the same passphrase to be allowed to connect.
- `--port <port for your daemon>` specifies which port the daemon should listen for incoming connections.

## Configuration files

If you keep starting Ethersync with the same options, you can also put any of these options into a configuration file at `.ethersync/config`:

```ini
peer = <multiaddr you want to try connecting to>
secret = <the shared secret>
port = <port for your daemon>
```
