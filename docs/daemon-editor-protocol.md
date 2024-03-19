---
date: 2024-03-19
---
# Reusable data types

```
DocumentUri = string // For example: "file:///home/user/bla/fu.txt"
DocumentRevision {
   uri: DocumentUri,
   revision: integer
}
Op: {range: Range, replacement: string}[]
Position: {line: number, character: number}
Range: {head: Position, anchor: Position}
```

# Editor to daemon

Before sending any of these messages, the editor needs to check whether the file belongs to a shared Ethersync directory. We can cache this information per buffer.

## `"open" {uri: DocumentUri}`

- Initializes editor revision to 0. Editor is interested in receiving updates!
- Takes ownership of file
- If this file didn't exist on the daemon before (and is in the project dir), it will create a "YDoc"

## `"close" {uri: DocumentUri}`

- This editor is no longer interested in receiving updates.
- Gives up ownership. When number of ownership reaches 0, daemon owns it again.

## `"edit" {doc: DocumentRevision, op: Op}`

- `revision` in the `DocumentRevision` is the last revision seen from the daemon.
- After each self-produced edit, the editor must increase its editorRevision.

## `"cursor" {doc: DocumentRevision, ranges: Range[]}`

- Sends current cursor position/selection(s). Replaces "previous one".

# Daemon to editor

## `"edit" {doc: DocumentRevision, op: Op}`

- `editorRevision` is the last revision the daemon has seen from the editor.
- If this is not the editorRevision stored in the editor, the editor must ignore the edit.
- The daemon will send an updated version later.

## `"cursor" {userid: integer, name?: string, doc: DocumentRevision, ranges: Range[]}`

- Daemon sends this always, independent whether relevant Document is open in the editor
    - to allow better presence to see where other people work
