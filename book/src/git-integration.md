<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Working with Git

While Ethersync currently doesn't have dedicated "Git integration" features, you can use it together with Git pretty well.

This section explains what possible *workflows* look like, and how Ethersync and Git concepts are interacting.

For the workflows below, we assume that you already have an established Git repository among the collaborating peers.

## Ignoring `.ethersync` directories

In Ethersync-enabled projects, you will have a directory called `.ethersync`.

If you always want to ignore these directories, you can add it to your global `.gitignore` file like this:

```bash
mkdir -p ~/.config/git/
echo ".ethersync/" >> ~/.config/git/ignore
```

## How Ethersync and Git interact

Ethersync tracks changes that you make to files in editors with an Ethersync plugin, and with [external tools](file-events.md).

However, any change to the `.git` directory and the staging area (which is in fact also tracked in the `.git` repository) is *ignored* by Ethersync. This means that Ethersync does not sync
- commits you create,
- files that you stage or unstage, or
- changes you're making to the HEAD.

This means that most Git operations you might try will not have an effect on connected peers.

### Git commands that don't modify files

These commands will not modify files, so you can run them without affecting connected peers (note that changes in your index or in your commits will not be shared):

- Checking what you have been doing so far with `git diff` / `git status`.
- Use `git add` and the like to stage changes.
- Use `git commit` to, well, create a commit in the current branch.

### Git commands that modify files

These commands might change file contents (without going through an editor). The file watcher should pick them up, but currently, they might trigger a deletion and re-creation of the affected files.
This is problematic if it happens in parallel to changes from other peers, or if they have them open in editors. We hope to build a smoother experience someday.

- Synchronizing with a remote repository with `git push` and `git fetch`.
- Use `git switch`/`git checkout` to switch to a different branch or get a specific file state from history.
- Use `git reset --soft` or `git reset --mixed` to modify the staging area and the HEAD "manually".
- Use `git restore`/`git checkout -- <pathspec>`/`git reset --hard HEAD` to undo your changes or get a different content of a file from the Git history.

If you notice any discrepancies between directory content of connected peers, you can turn off the daemon, and restart it. The daemon will then pick up changes, as described in the section about [offline support](offline-support.md).

## Recommended pair-programming workflow

When you start the daemon, make sure all peers are starting on the same commit with a clean staging area.

When you want to make a commit together, all peers should stop typing/editing files, then, *one* person should create the commit:

### Committer

As a committer, create a commit like you usually would, and push it to a remote repository:

```bash
git push
```

### Other peers

1. Any other peer can then fetch the changes without applying them. Note: The changes *are* already applied to their working tree, through Ethersync.

    ```bash
    git fetch
    ```

2. Now each peer can update the HEAD. The easiest option is to run:
    
    ```bash
    git reset @{u}
    ```

    What does this command do?

    - `git reset` will move your current branch to the given commit, and also set your index to the content of that commit. Notably, it does *not* touch the working directory (because it already contains exactly the content we want).
    - `@{u}` is an abbreviation for the upstream branch which the current branch is tracking. For example, it could mean `origin/main` (but it always refers to the correct upstream branch).

    As an effect, this command brings you to the same Git state like the committer.

All peers can then use `git status`/`git diff` to double check that they have the same diff now (if the commit contains all changes, the diff should be empty).

## Recommended note-taking workflow

In the note taking use case, you can use Git for keeping your own local backup copy of the note's contents. You can then use it, to track which parts have been changed by others, for example while you were offline.

Let's say you have initially added and committed all notes.
- Whenever you are reconnecting to the cloud peer and are getting some changes, you can revise them by looking at the git diff.
- Then you can add and commit them with an unimportant commit message to set a "savepoint" for next time

It's also a nice little back-up in case anything goes wrong with the sync. Which might happen given that this is very new and bleeding edge software, be it through bugs or misunderstandings.
