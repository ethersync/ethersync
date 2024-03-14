---
status: draft
date: 2024-03-14
---
# Who saves files?

## Context and Problem Statement

Usually, when opening a file in a text editor, the content is read into its buffer. The user then works on the buffer, and sometimes writes it back to the file.

In an Ethersync scenario, we might need a different approach.

The question is: Who writes to files, and when? Who has "ownership" of the files?

## Decision Drivers

* It should feel as natural as possible to work in a shared directory.
* It should be possible to work on the files using external tools.

## Considered Options

When editors have opened files:

* As soon as editors open a file, the file content is considered irrelevant/ignored/unspecified from daemons perspective. Editor has ownership. The daemon doesn't write to the files.
    * assumes that no other service/program is invested into the file content, everything happens in the buffer
        * maybe we can get around the filesystem by providing another "interface" to the content, through ethersync tooling.
    - External programs can't see the current content of the file.
    + Under regular usage, editors don't complain about changed file contents.
    o When file is open in more than one editor, and user saves, editors will still complain about changed files.
    Note: Maybe implement a ping mechanism, so that the daemon can detect when editors have crashed?
* Try to keep the file up-to-date with the "true" CRDT content
    -> hard to keep editors from complaining about the writes
        => excludes this option relatively hard

When no editor has the file open:

* Daemon keeps the content up-to-date with its CRDT content immediately
* Daemon *sometimes* writes the current state, has it's own caching/writing logic
    * decide on a middle-ground default, but make it configurable to the user preferences?
    * could change dependent on whether the user is "active" (i.e. has other files open)
    * we open a filewatching process that computes diffs and thus applies changes
* Don't write to files
    -> Not good

---

When should editors write?

* E1: They auto-save their content immediately after each keystroke.
* E2: They save as normal, whenever the user wants to.
* E3: They never write to the file.

When should the daemon write?

* D1: As soon as it updates its CRDT, it write to the file.
* D2: When editor has ownership, never. When no editor has ownership, immediately.
* D3: Never!
* D4: When the daemon shuts down.
* D5: When the daemon is triggered to do so

When should the daemon read/interpret file changes?

* Always
* When no editor is open
* Never

What is the truth?

* The content of the CRDT. Seems like it.
* The content of the file.

|----|----|----|----|
|    | E1 | E3 | E3 |
|----|----|----|----|
| D1 |    |    |    |
| D2 |    |    |    |
| D3 |    |    |    |
|----|----|----|----|

E1
    - Two open editors could be in conflict
    - A lot of writes (one per keystroke)
E3
    - Hard to prevent saves...

D1
    - Editor will be confused by demon writes
    + Content is set to the "truth", good for external tools
D2
    + daemon does not irritate editor

E1 & D1
    Both write as fast as they can :D
    o Editor and CRDT will be close to each other anyway, so what they want to write is often similar
    -- When daemon writes, there will be brief periods of time where the editor is confused by an outside write.
    - A lot of writes (two per keystroke)
E1 & D2
    Editor writes after each change, daemon only when editor is not open
    + No conflicting writes by the daemon, editor is happier
    + File is always close to the CRDT content
E1 & D3
    Editor writes after each change, file is not updated when no editor is open
    - very old content can be on disk, available to `cat`, for example. Would prevent you from working on files using external tools.

E2 & D1
    Editor writes sometimes, daemon after each change
E2 & D2
    Editor writes sometimes, daemon only when editor is not open
    - file is not close to truth when the user doesn't save, and has the file open
E2 & D3
    Editor writes sometimes, daemon never
    - two editors can do conflicting writes
    - very old content can be on disk, available to `cat`, for example
    + close to regular use

E3 & D1
    Editors never write, daemon as fast as it can
    + single source of writes
E3 & D2
    Editors never write, daemon only when editors not open
    o content of files is close to truth when editors are closed
E3 & D3
    Nothing ever write :|
    - the file doesn't need to be there :D

Do we want to allow external tools be used on files that are open in editors?

Do we want two (different) editors to edit the same file?
    -> If so, 

Can we prevent all editors from being unhappy when its open file is changed under its nose?
    Vim
        Normally warns: "Modified by Vim and outside"
        Maybe set 'nowrite'? Or the buftype "nofile"?
    Emacs
        Yes https://stackoverflow.com/questions/2284703/emacs-how-to-disable-file-changed-on-disk-checking
        Seems to even lock open files, and prevent the second instance form editing
    VS Code
        Yes https://github.com/microsoft/vscode/issues/66035
            files.saveConflictResolution
        Normally, on save: "Failed to save: content of the file is newer."
            Offers to open diff
    Jetbrains
        "changes have been made" "in memory and on disk"

Two reponses to changed files:
    Show warning immediately
    Show warning on save

## Decision Outcome

Chosen option: "{title of option 1}", because
{justification. e.g., only option, which meets k.o. criterion decision driver | which resolves force {force} | … | comes out best (see below)}.

<!-- This is an optional element. Feel free to remove. -->
### Consequences

* Good, because {positive consequence, e.g., improvement of one or more desired qualities, …}
* Bad, because {negative consequence, e.g., compromising one or more desired qualities, …}
* … <!-- numbers of consequences can vary -->

<!-- This is an optional element. Feel free to remove. -->
## Validation

{describe how the implementation of/compliance with the ADR is validated. E.g., by a review or an ArchUnit test}

<!-- This is an optional element. Feel free to remove. -->
## Pros and Cons of the Options

### The daemon owns all synced files

Editors should never write to a file.

Vim has 'nowrite' option.

Means: Save for each keystroke, probably?

Can maybe handle external tool writes.

<!-- This is an optional element. Feel free to remove. -->
{example | description | pointer to more information | …}

* Good, because {argument a}
* Good, because {argument b}
<!-- use "neutral" if the given argument weights neither for good nor bad -->
* Neutral, because {argument c}
* Bad, because {argument d}
* … <!-- numbers of pros and cons can vary -->

### As soon as an editor opens a file, ownership is transferred to it

What happens when a second editors *wants* to open it?
    Daemon could tell editor to open read-only.

{example | description | pointer to more information | …}

* Good, because {argument a}
* Good, because {argument b}
* Neutral, because {argument c}
* Bad, because {argument d}
* …

### Co-ownership for all editors which open a file

What happens when an editor writes?
    -> Ignored?

<!-- This is an optional element. Feel free to remove. -->
## More Information

When we keep editor synced, their buffer content is always the "true" content (from the perspective of the editor), so saving doesn't make a lot of sense.
In "offline mode", saving could be the mechanism to share edits!

What happens when a file is open in an editor, but an external tool writes to it at the same time? Ideally, the file should be updated in the editor!

What happens when two editors open the same file? They should be kept in sync.
    When both write, those writes would need to be ignored.

Isn't an editor just another "external tool" that can write to files?

How can the daemon process file edits?
    Examples:
        `echo new line >> file`
        `sort -o file file`
    Problem: We won't preserve the "meaning" of inserting at the end
        Instead, the op is: insert(4321, "new line")
    If we get file watcher notification that file content changed:
        We know the previous state, because we are notified of *every* write
            So we know the diff
            And the timestamp of each write
    Concurrent tool edits can still happen, and will always be a problem...
        "last write wins"?
How probable is it that multiple editors have a file open?
Responsibility on the user's side?
How does editors' "file changed warning" work? Can we catch it?
Two types of read-only
    Can't save to files
    Can't modify buffer

Can it happen that file edit events are notified after daemon did a concurrent write?
File locking
    mandatory and advisory locking
    lslocks
    flock
    "mandatory locks are generally virtual file systems"
inode handling
Which mechanism does lsof use?
    "files opened by processes"
preventing writes for external tools seems bad
Do file change notifications contain content?
check for change + write atomic?
-> do as best as we can?
    we only have to be as good as current situation, without collaboration

How would it be if file systems didn't exist at all?
    Editors would request file content from the daemon
    Editors wouldn't need to save
How to handle the situation where daemon/another editor writes a file while open in editor?
    Can we disable warnings?
    Disallow it while editors are open?
What happens in the LSP world if an editor wants ownership, but another editor already has it?

Some file systems have "access timestamps"!

Conflicting writes with two open editors are a problem anyway, without Ethersync.

Observation that external tools in the workflow can be
- read only (like a compiler)
- read-write (like a awk/formatter)
- write-only (log appending)
