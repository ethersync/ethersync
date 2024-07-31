---
date: 2024-03-19
updated: 2024-07-09
---

This document describes the protocol between the Ethersync daemon and the text editors. It should contain everything you need to implement a plugin for a new editor!

# Introduction to the architecture and mechanisms

The Ethersync daemon is the process that has the task to synchronize with others, the job of the editor is to communicate changes to the daemon. To make this as easy as possible for the plugin, we took some inspiration of how LSP Language Servers work. So if your editor plugin system allows connecting to an LSP, you'll hopefully don't have to think about the underlying mechanisms. Still, it doesn't hurt to know what's going on, so here's a little demo:

## LSP and JSON-RPC

Editors communicate with [LSP servers](https://microsoft.github.io/language-server-protocol/overviews/lsp/overview/) through [JSON-RPC](https://www.jsonrpc.org/specification).

We're using a little piece of software, internally called "jsonrpc-forwarder", which is executed via `ethersync client`.
It connects to the daemon and can be interacted with through stdin/stdout.

Think of `ethersync client` as the LSP Server when looking at it from the editor's perspective.

### Example message to the daemon

To see the direction from this client to the daemon you can try the following. Assuming you can start the daemon on a playground similar to the [README](../README.md), now we add some debugging output:
```bash
ethersync daemon playground -d
# Note for below: You will see some output like "Listening on UNIX socket: /tmp/ethersync"
```
You can then, in another terminal start the client:
```bash
ethersync client
```
This will already produce an output in the daemon which indicates that an Editor connected.
This happens, because the client connects to the /tmp/ethersync socket.
Killing it shows the opposite "editor disconnected".

Next, you could manually send some JSON-RPC. As the client is a bit picky on the protocol, we'd recommend scripting it.

If you have python installed, we have a tiny tool for you. If not, feel free to use it as a reference :)
```bash
python tools/dummy-jsonrpc.py playground/file | ethersync client
```


### Example seeing what Neovim communicates

On the other hand, not running any daemon, you can see what the plugin "wants" to communicate as follows.
In the demon console (stop it), we now just plainly listen on the socket for incoming data:
```bash
# nc can only bind to existing sockets, so we'll drop potentially existing ones
rm /tmp/ethersync; nc -lk -U /tmp/ethersync
```

In the client console, start nvim on a file, move the cursor and edit something:
```bash
nvim playground/file
```

# File ownership

By default, files in an Ethersync directory are owned by the daemon. The daemon can directly write updates to them.
When an editor sends an "open" message, it takes ownership; all changes to the file by other sources will now be sent through it.

When the last editor gives up ownership by sending a "close" message, the daemon takes ownership again.

# Editor revision and daemon revision

For each open file, editors store two integers:

- The *editor revision* describes how many changes the user has made to the file. It needs to be incremented after each edit made by the user.
- The *daemon revision* describes how many changes the editor received from the daemon. It needs to be incremented after receiving an edit from the daemon.

# Basic data types

The protocol uses a couple of basic data types:

- `DocumentUri = string`

    This is an absolute file URI, for example: "file:///home/user/bla/fu.txt"`.

- `Position: {line: number, character: number}`

    A position inside a text document. Characters are counted in Unicode characters (as opposed to UTF-8 or UTF-16 byte counts).

- `Range: {start: Position, end: Position}`

    A range inside a text document. For cursor selections, the *end* is the part of the selection where the active/movable end of the selection is.

- `Delta: {range: Range, replacement: string}[]`

    A complex text manipulation, similar to LSP's [`TextEdit[]`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textEditArray). Like in LSP, all ranges refer to the starting content, and must never overlap, see the linked LSP documentation.

- `RevisionedDelta: {delta: Delta, revision: number}`

    This attaches a revision number to a delta. The semantics are that the delta *applies to* (is intended for) that specified revision.

# How the editor recognize Ethersync-enabled directories

Similar how Git repositories have a `.git` directory at the top level, Ethersync-enabled directories have an `.ethersync` directory at the top level. The editor must only send messages for files inside Ethersync-enabled directories.

# Messages sent by the editor to the daemon

## `"open" {uri: DocumentUri}`

- Sent when the editor opens a document. By sending this message, the editor takes ownership of the file, and tells the daemon that it is interested in receiving updates for it.
- The editor has to initialize its editor revision and daemon revision for that document to 0.

## `"close" {uri: DocumentUri}`

- Sent when the editor closes the file. It is no longer interested in receiving updates.

## `"edit" {uri: DocumentUri, delta: RevisionedDelta}`

- The `revision` attribute of `RevisionedDelta` is the last revision seen from the daemon.
- After each user edit, the editor must increase its editor revision.

## `"cursor" {uri: DocumentUri, ranges: Range[]}`

- Sends current cursor position/selection(s). Replaces the previous cursor ranges.

# Daemon to editor

## `"edit" {uri: DocumentUri, delta: RevisionedDelta}`

- `revision` in the `RevisionedDelta` is the last revision the daemon has seen from the editor.
- If this is not the editor revision stored in the editor, the editor must ignore the edit. The daemon will send an updated version later.
- After applying the received edit, the editor must increase its daemon revision.

## `"cursor" {userid: integer, name?: string, uri: DocumentUri, ranges: Range[]}`

- The daemon sends this message when user's cursor positions or selections change, regardless of whether the file has been opened in the editor. The editor can use this information to display in which files other people work.
