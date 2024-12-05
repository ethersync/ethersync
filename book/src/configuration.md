# Configuration

There are two ways to configure what an Ethersync daemon will do.

## Command line flags

You can provide the options on the command line, after `ethersync daemon`:

- `--peer <multiaddr>` specifies which peer you want to try connect to.
- `--port <port for your daemon>` specifies which port the daemon should listen for incoming connections.

For security reasons, it is not possible to provide the secret via a command line flag.

## Configuration files

If you keep starting Ethersync with the same options, you can also put the following options into a configuration file at `.ethersync/config`. Here, you have the option to specify a secret; otherwise, a default password will be used, which is only recommended for testing:

```ini
peer = <multiaddr you want to try connecting to>
port = <port for your daemon>
secret = <the shared secret>
```
