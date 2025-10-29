<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# 0.9.0 (2025-10-29)

Teamtype (previously called Ethersync) enables real-time collaborative editing of local text files, with plugins for Neovim and VS Code.

## Breaking change: Renamed from Ethersync to Teamtype

This release exists mostly because we decided to rename the project! It used to be called "Ethersync", derived from software like EtherPad and SubEthaEdit. But ultimately, we think the name "Teamtype" will serve the project better, because of a couple of reasons:

- It's more self-descriptive.
- It highlights the collaborative aspect.
- It avoids associations with "Ethereum".
- It's easier to pronounce in English, German, and hopefully other languages as well.
- When we made the decision a couple of months ago, it was completely unoccupied on the Internet.
- We got a great domain!
- It's already crowd-validated, because in [a poll](https://pol.is/report/r74mttmj5vrbh6c7hm64h) we did last year, it was in the top 5 of the suggestions.

We hope you like the new name as much as we do.

Because of the name change, old versions of the Ethersync daemon and the plugins are no longer compatible with this release.

### Upgrade instructions from 0.8.0

- Install the `teamtype` daemon binary via some method as described in the README.
- Uninstall the old editor plugins, and then install the new Teamtype plugins.
- Updating the local metadata: When you first run `teamtype share/join`, the program will offer you to rename `.ethersync/` to `.teamtype/` automatically. This will happen for every previously shared project independently.

## Note for package maintainers: Shell completions and man pages

Thanks to @EdJoPaTo, `cargo build` now generates shell completion files (in `target/completions/`) and man pages. It'd be great to include/install them! Thank you for maintaining the package :)

## Make transmission of file changes less noisy

When pair programming on a software repository, using commands like `git checkout <file>` used to be a bad experience, because it would first remove the file from the CRDT document, and then re-create it. This led to a lot of noise in the logs, and to less stable collaboration on these files.

We rewrote our file watcher to "de-bounce" the observed file events. When a file disappears, the watcher will now wait for 100 ms, and if it re-appears in that time, only the diff is communicated to the peers.

## Experiment: Smoothly collaborate on Git repositories

We have added a flag `--sync-vcs` to enable synchronization of Git and other Version Control Systems' local data. It allows working closely together when pair programming: Creating commits (and even writing the commit messages collaboratively!), switching branches, etc.

As mentioned in many places: This might possibly corrupt your local `.git` directory, so beware and use with caution. We can recommend it as a fun experiment to see how it feels to break up a common pattern of previously asynchronous workflows.

## Other improvements in this release

- In the Neovim plugin, the follow mode automatically stops whenever any key is pressed (thanks, @MichaelBitard).

# 0.8.0 (2025-09-26)

Ethersync enables real-time collaborative editing of local text files, with plugins for Neovim and VS Code.

Our 0.7.0 release got a lot of attention, and this repository quickly reached 1000 stars on GitHub! We also did two Community Calls, so it has been a busy month!

Here's whats new in this release:

## Breaking change: "open" message now has a "content" field

When an editor opens a file, it now has to send along the current file content. The messages now look like this:

```json
{"jsonrpc": "2.0", "id": 1, "method": "open", "params": {"uri": "file:///path/to/project/file", "content": "initial content"}}
```

We added the "content" field to avoid race conditions, where the file content changes after the editor has loaded the content into its buffer. This now makes it safe to open the file at any point, even if other people are currently typing in it. We took inspiration from LSP's [`textdocument/didOpen`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textDocument_didOpen) message. You can find more information in the [editor plugin developmend guide](https://ethersync.github.io/ethersync/editor-plugin-dev-guide.html).

## Binary file support

Ethersync now will also sync binary files that are in a project. Previously, they were just ignored. This allows you to collaborate on projects that involve image files, for example, so you could do web or game development together!

The synchronization algorithm works in a way that the "latest write" of the same file will win in case of a conflict, essentially.

While you *could* now use Ethersync synchronize your photo galleries between computers (and in our tests, synchronizing video files that are over 1 GB large seemed to work), note that this is definitely not our main use case (check out [Syncthing](https://syncthing.net) for an alternative).

## Follow mode for Neovim

@MichaelBitard contributed a "follow mode" for the Neovim plugin: You can run `:EthersyncFollow` to follow another person, and see what they're editing -- even if they're switching files, you will follow them. Run `:EthersyncUnfollow` to stop following them. These are super convenient, and we'd recommend creating mappings for them (see the [ethersync-nvim](https://github.com/ethersync/ethersync-nvim/tree/develop) repo for examples).

## Automatically reconnecting to peers

Once you've established a connection to another peer (by using `ethersync join`), Ethersync will now try to keep this connection active. This means that, if the peer temporarily goes offline (for example because they're in a train that's going through a tunnel), the connection will be re-established once they are reachable again.

This allows a better "launch once and forget" usage.

## Update to Automerge 1.0 - reduced memory usage

We updated Automerge (the library that we use for synchronizing edits without a central server) to version 1.0.0! (1.0.0-*beta.3*, to be precise.) This brings, as [their announcement puts it](https://automerge.org/blog/automerge-3/), dramatic reduction in memory-usage, in an entirely backwards-compatible way. In our tests, memory usage went down from ~1 GB to 180 MB, in one case, and document loading times are also improved by about half!

Note that the linked announcement describes the JavaScript library for Automerge, which was updated to version 3.0, while we're using the foundational Rust library directly, which is being updated to version 1.0.

## Official packages for Arch Linux and nixpkgs

There are now official packages for [Arch Linux](https://archlinux.org/packages/extra/x86_64/ethersync/) (maintained by @svenstaro and @alerque), and in the [nixpkgs](https://search.nixos.org/packages?channel=unstable&show=ethersync) (maintained by the NGI team). These should make it easier to install Ethersync for many people. Thanks!

## Configuring the plugins for arbitrary collaboration software

Inspired by a collaboration with [Braid](https://braid.org) (a project that extends HTTP as a state synchronization protocol), we wanted to make it possible to use our Neovim and VS Code plugins with other collaborative software. You can now configure the plugins to work with any program that speaks our [editor protocol](https://ethersync.github.io/ethersync/editor-plugin-dev-guide.html). This works similarly to how you can configure Language Servers in your editors.

Configurations to connect with the regular Ethersync daemon are provided and enabled by default, so if you don't need this configurability, you don't need to do anything.

For example, here's a Neovim configuration block to launch a (fictional) `ethersync-http` "collaboration server" for any buffer that starts with "https://":

```lua
ethersync.config("http", {
    cmd = { "ethersync-http" },
    root_dir = function(bufnr, on_dir)
        local name = vim.api.nvim_buf_get_name(bufnr)
        if string.find(name, "https://") == 1 then
            on_dir("/tmp")
        end
    end,
})
ethersync.enable("http")
```

For VS Code, you can add a setting like this, based on "root markers" (files or directories that must be in the root of your project directories):

```jsonc
"ethersync.configs": {
  "http": {
    "cmd": [ "ethersync-http" ],
    "rootMarkers": [ ".ethersync-http" ]
  },
}
```

An actual prototype for another program that speaks our protocol is @dglittle's [braid-ethersync](https://github.com/braid-org/braid-ethersync) bridge.

You can find in-depth documentation about this in the [VS Code plugin README](https://github.com/ethersync/ethersync/tree/main/vscode-plugin) and new [Neovim help file](https://github.com/ethersync/ethersync-nvim/blob/develop/doc/ethersync.txt).

## Other improvements in this release

- @edjopato enabled a lot of lints for [Clippy](https://github.com/rust-lang/rust-clippy) in our Rust code, and introduced dozens of tiny refactorings.
- @aileot made sure that the Neovim code is in an "ethersync" namespace, instead of exporting functions at the top level.
- To avoid issues where the file watcher sometimes missed newly created files, we're now doing full re-scans of the project directory after any file events.
- Daemons use a UNIX socket in `.ethersync/socket` to communicate with the plugins. They now remove this socket after a clean exit. Which means that we can now warn you if you're trying to start *two* daemons for the same project! You get asked whether or not you want to continue the launch of the second daemon. This can also happen if the first daemon crashed.
- We deploy the Neovim plugin to the main branch on [ethersync-nvim](https://github.com/ethersync/ethersync-nvim) only for releases, which means if you install it from there, you'll always get a stable version. If you want to use the latest release, you can install the plugin from the "develop" branch.
- When a peer deletes a file, but another peer has that file open in an editor, the file will immediately be re-created, but it will be empty. This allows a smooth handling of this case - the second peer can keep typing into the file.
- In addition to `.git`, we're now ignoring plenty of other version control directories (and don't share their content to peers): `.bzr`, `.hg`, `.jj`, `.pijul`, `.svn`
- We're now providing a binary for ARM64 devices in each release. These are mainly meant to be used on Android! We recommend using it from within the [Termux](https://termux.dev) terminal emulator. The target triple is `aarch64-unknown-linux-musl`
- We're trying a better method of recognizing remote edits that are sent to the VS Code plugin, after finding out that VS Code tries to do some clever "minification" of the edits it receives. This approach isn't perfect yet, and you might occasionally run into inconsistencies, unfortunately. We'll keep iterating on this!

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
