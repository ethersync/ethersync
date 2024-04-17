---
date: 2024-03-19
updated: 2024-04-17
---
# Reusable data types

## `DocumentUri = string // For example: "file:///home/user/bla/fu.txt"`

## `Delta: {range: Range, replacement: string}[]`

A complex text manipulation, similar to LSP's [`TextEdit[]`](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/#textEditArray). Like in LSP, all ranges refer to the starting content, and must never overlap, see the linked LSP documentation.

## `RevisionedDelta: {delta: Delta, revision: number}`

## `Position: {line: number, character: number}`

## `Range: {head: Position, anchor: Position}`

## `RevisionedRanges: {ranges: Range[], revision: number}`

# Editor to daemon

Before sending any of these messages, the editor needs to check whether the file belongs to a shared Ethersync directory. We can cache this information per buffer.

## `"open" {uri: DocumentUri}`

- Initializes editor revision to 0. Editor is interested in receiving updates!
- Takes ownership of file
- If this file didn't exist on the daemon before (and is in the project dir), it will create a "YDoc"

## `"close" {uri: DocumentUri}`

- This editor is no longer interested in receiving updates.
- Gives up ownership. When number of ownership reaches 0, daemon owns it again.

## `"edit" {uri: DocumentUri, delta: RevisionedDelta}`

- `revision` in the `RevisionedDelta` is the last revision seen from the daemon.
- After each self-produced edit, the editor must increase its editorRevision.

## `"cursor" {uri: DocumentUri, ranges: RevisionedRanges}`

- Sends current cursor position/selection(s). Replaces "previous one".

# Daemon to editor

## `"edit" {uri: DocumentUri, delta: RevisionedDelta}`

- `revision` in the `RevisionedDelta` is the last revision the daemon has seen from the editor.
- If this is not the editorRevision stored in the editor, the editor must ignore the edit.
- The daemon will send an updated version later.

## `"cursor" {userid: integer, name?: string, uri: DocumentUri, ranges: RevisionedRanges}`

- Daemon sends this always, independent whether relevant Document is open in the editor
    - to allow better presence to see where other people work
