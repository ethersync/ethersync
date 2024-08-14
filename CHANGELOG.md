# Unreleased features

(None yet)

# 0.3.0 (2024-08-13)

New features:

- Integrated libp2p for transport encryption and authentication of peers.
- Added a password protection – peers must specify the same `--secret` to connect.
- You can now provide default startup arguments in `.ethersync/config`.
- When editors send

Bug fixes:

- The daemon is now more resilient against misbehaving peers and editors, and won't crash as easily.

# 0.2.2 (2024-08-01)

New features:

- Persist the CRDT to `.ethersync/doc`, so that peers can run independently, and sync back later.
- When restarting the daemon, calculate file diffs since the daemon was last online, and apply them to the CRDT.
- Dynamically create files when they are opened.
- When files are deleted, delete them for all peers.
- Sandbox file I/O to be restricted into the shared project folder.

# 0.2.1 (2024-07-26)

New features:

- Share multiple files per directory.
- Transmit cursor positions, and display them in the Neovim plugin.

# 0.2.0 (2024-05-02)

Rewrite of the initial prototype in Rust.

# 0.1.0 (2024-01-22)

An initial prototype in Typescript. It only supports sharing a single file.
