---
status: accepted
date: 2024-03-14
---
# Who saves files?

## Context and Problem Statement

Usually, when opening a file in a text editor, the content is read into its buffer. The user then works on the buffer, and sometimes writes it back to the file.

In an Ethersync scenario, we could follow a different approach, where the daemon also writes to files.

The question is: Who writes to files, and when? Who has "ownership" of the files?

## Decision Drivers

* It should feel as natural as possible to work in a shared directory.
* It should be possible to work on the files using external tools, like `awk`, or appending things to the files.

## Considered Options

When a file has been opened in editors:

* The daemon keeps an opened file content up-to-date with the CRDT content.
* When editors open a file, they take ownership of it. The daemon doesn't write the file.

When no editor has the file open:

* The daemon keeps the files up-to-date with its CRDT content immediately
* The daemon writes the current state occasionally, with its own caching/writing logic
* The daemon never writes to the files

## Decision Outcome

Chosen option: "When editors open a file, they take ownership of it. When no editor has the file open, the daemon writes to it using its own logic"

Notes:

- We could provide another "interface" to the content, through ethersync tooling, like `ethersync run "sed -i s/foo/bar file"`.
- Maybe we should implement a ping mechanism, so that the daemon can detect when editors have crashed.
- When a file is opened in more than one editor at once, there will be conflicting writes anyway. This is true without a software like Ethersync. The only way to prevent that would be to stop editors from writing at all, which doesn't seem desirable.
- To incorporate changes done by external tools, we watch the file system, compute diffs, and apply changes to the CRDT. This seems true both when the file is open or closed in editors.

## Pros and Cons of the Options

### The daemon keeps an opened file content up-to-date with the CRDT content.

* Good, because the file content is close to the CRDTs "truth" at all times, which means that external files will see up-to-date content.
* Bad, because it will often happen that the daemon changes the file content without involvement of the editors. Editors are usually not happy about that, especially when they have made edits themselves in the meantime. They tend to complain. Fixing that in all plugins seems hard. This excludes this option.

### When editors open a file, they take ownership of it. The daemon doesn't write to the file.

The daemon doesn't write to the file when it's opended in an editor, but the editors can, whenever the user wants. The "true" CRDT content is in the editor's buffer.

* Bad, because the file content is unspecified in this situation, which means that external tools will see old versions. This could be improved by auto-saving features.
* Good, because editors won't complain about changed file contents.

### The daemon keeps closed files up-to-date with its CRDT content immediately

* Good, because the latest content is always visible to external tools.
* Bad, because writing for each keystroke might damage hard drives.

### The daemon writes the current CRDT state to closed files occasionally, with its own caching/writing logic

Ethersync could decide on a default update interval, but make it configurable to the user preferences. The write intervals could change dependent on whether the user is "active" (i.e. has other files open)

* Good, because it strikes a middle ground between damaging the hard drive by writing to often, and neglecting external files by not updating the file at all.
* Neutral, because its a bit more complex than the other options.

### The daemon never writes to closed files

The files would only be updated when opened in editors.

* Very bad, because then external tools would never see up-to-date content.

## More Information and brainstorming notes

What happens when a file is open in an editor, but an external tool writes to it at the same time? Ideally, the file should be updated in the editor! In our chosen solution, we don't have that property.

How would it be if file systems didn't exist at all? Editors would request file content from the daemon, and they wouldn't need to save.

What happens in the LSP world if an editor wants ownership, but another editor already has it? Can the LSP reject the opening?

Observation that external tools in the workflow can be:
- read only (like a compiler)
- read-write (like a awk/formatter)
- write-only (log appending)

How to handle the situation where daemon/another editor writes a file while open in editor? Can we disable warnings?

- Vim
    Normally warns: "Modified by Vim and outside"
    Maybe set 'nowrite'? Or the buftype "nofile"?
- Emacs
    Yes: https://stackoverflow.com/questions/2284703/emacs-how-to-disable-file-changed-on-disk-checking
    Seems to even lock open files, and prevent the second instance form editing
- VS Code
    Yes: https://github.com/microsoft/vscode/issues/66035
        files.saveConflictResolution
    Normally, on save: "Failed to save: content of the file is newer."
        Offers to open diff
- Jetbrains
    Warns: "changes have been made" "in memory and on disk"
    Not clear how to turn that off

Currently, editors use two possible reponses to changed files:
    Show warning immediately
    Show warning on save
