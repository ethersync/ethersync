<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# 0.7.0 (2025-07-27)

Update instructions for plugin authors:

- In order to find the correct socket to connect to, `ethersync client` now needs to know the directory which contains the `.ethersync/` subdirectory. Search up from the edited file path to find it, and then either change the current directory there, or use the `--directory` command-line option to specify it.

Other breaking changes:

- Switched from libp2p to [iroh](https://www.iroh.computer) as our peer-to-peer transport library. This changes the format of the connection addresess, and the underlying protocol.
- Redesigning the CLI UI: Remove the `daemon` subcommand, instead use `share` and `join`.
- Use `.ethersync/socket` as the location for the daemon's socket. This makes it possible to easily run multiple daemons at the same time.
- Remove `--socket-name` command-line option.
- The canonical location of the Neovim plugin is now at <https://github.com/ethersync/ethersync-nvim>. The directory from this repository is mirrored there.

Bug fixes:

- Neovim plugin: When pressing `o` on a line, place edit in the new line, instead of after the old one. This avoids moving other people in the old line down.
- Neovim plugin: Don't send out deltas that don't change anything

New features:

- Use [magic-wormhole.rs](https://github.com/magic-wormhole/magic-wormhole.rs) for making initial connections. This allows you to easily tell a short joincode to another person.
- Make cursor messages ephemeral, and don't store them in the CRDT document. This makes the document smaller, and loading and syncing faster.
- Infer pair-programming use-case when there are configured Git remotes. In that case, always start a new history.
- Always write current content to files, instead of respecting editor ownership. This allows better paring on Rust and web projects.
- Neovim plugin: Add support for Neovim 11
- Add global `--directory` command-line option to `ethersync`, which allows you to set the shared directory without changing your current path there.
- The name shown with the cursor is now picked up from your (global or local) gitconfig instead of `$USER`.

# 0.6.0 (2024-12-13)

Breaking changes:

- The command-line option `--socket-path` was removed and `--socket-name` added (for security reasons, see below).
- The command-line option `--secret` was removed (see also "Security improvements")

Security improvements:

- Make sure that the socket file is always in a directory only accessible by the current user.
- Remove the `--secret` command-line option, to avoid people from leaking their secret on multi-user systems.
- Force the keyfile permissions to be restricted to the user, to prevent leaking it on a multi-user system.

Bug fixes:

- Avoid crashes in the daemon in cases where files appear and quickly disappear again.
- Neovim plugin: Prevent reloading changed files from disk, which can lead to inconsistent content between peers, by forcing the 'autoread' option off.
- VS Code plugin: Send out UTF-8 file path in messages to daemon, instead of using percent encoding, to keep compatibility with our existing assumptions.
- VS Code plugin: Auto-save files to prevent data loss when closing without saving.

New features:

- When failing to connect to a peer (or when a peer fails to connect) show an error message.
- VS Code plugin: Provide a command to show other cursor positions (for accessibility).

# 0.5.0 (2024-09-30)

Breaking changes:

- "edit" messages now contain the revision and the delta on the top level, to avoid unnecessary nesting.

New features:

- We have installable packages on crates.io and on the Arch User Repository.
- Lower mimimum supported Neovim version from 0.10 to 0.7.
- Released a VS Code plugin!
- When there is no persisted CRDT document, load a structure that's compatible with other peers. This allows peers to start up individually, and sync up later.
- Persist changes to CRDT incrementally, instead of saving the entire document each time. This gives a big performance boost.

Bug fixes:

- Does not crash when binary files exist by ignoring them.

# 0.4.0 (2024-09-13)

New features:

- `ethersync client` now connects to the socket specified in the environment variable `ETHERSYNC_SOCKET`.
- Published extensive user documentation at <https://ethersync.github.io/ethersync/>.
- Published ADR-08 (on secure connections) and ADR-09 (on the data structure for multiple files).
- Support multiple local editors connecting the the same daemon. Caveat: Their initial state needs to be in sync.
- Watch for file creation and modification by external tools in project directory, and forward these changes to the peers.

Bug fixes:

- When peers put something invalid into the "states" key of the CRDT, ignore it.
- Fix bug when inserting text ending with \n after last visible line. This happened, for example, when performing a `echo line >> file`.

# 0.3.0 (2024-08-13)

New features:

- Integrated libp2p for transport encryption and authentication of peers.
- Added a password protection – peers must specify the same `--secret` to connect.
- You can now provide default startup arguments in `.ethersync/config`.

Bug fixes:

- The daemon is now more resilient against misbehaving peers and editors, and won't crash as easily.

# 0.2.2 (2024-08-01)

New features:

- Persist the CRDT to `.ethersync/doc`, so that peers can run independently, and sync back later.
- When restarting the daemon, calculate file diffs since the daemon was last online, and apply them to the CRDT.
- Dynamically create files when they are opened.
- When files are deleted, delete them for all peers.
- Sandbox file I/O to be restricted into the shared project folder.
- The editor plugin can now send JSON-RPC requests and get feedback on whether the intended operation worked or not.

# 0.2.1 (2024-07-26)

New features:

- Share multiple files per directory.
- Transmit cursor positions, and display them in the Neovim plugin.
- Add a Nix flake to the project for simplified installation.
- The "cursor" messages in the editor protocol are no longer revisioned. In practice, this seems to work well enough.

# 0.2.0 (2024-05-02)

Rewrite of the initial prototype in Rust.

# 0.1.0 (2024-01-22)

An initial prototype in Typescript. It only supports sharing a single file.
