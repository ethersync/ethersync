# Working with Git

## When Pair Programming

While we have some vague ideas on how actual Git support could look like, there's currently no real integration.

This section, however, lays a foundation and then explains how a possible *workflow* looks like and how the Ethersync and Git concepts are interacting when you're working on a Git-based project.

If you don't want to know all the details you could also jump ahead to the last section and just apply the workflow.

### Recap: What Git is doing under the hood

In case you're not a power-user of Git, it might be helpful to review some concepts here, in order to build upon them.

Mainly it's important to understand the three sections of a Git repository and when you're changing which of them:
- The working tree
- The staging area
- The Git directory

This picture, from the "The Three States" section of ["What is Git?"](https://git-scm.com/book/en/v2/Getting-Started-What-is-Git%3F) in the Git Book illustrates these three sections:
![Three sections of a Git repository and their transitions](git-integration-areas.png)

(Editor's note: Another helpful perspective is https://git-scm.com/book/en/v2/Git-Tools-Reset-Demystified, which talks about "The Three Trees". Maybe that's more in the direction that we want to go, as "The Three States" rather talks about modified, staged, and committed, which is not *that* helpful here)

It's good to have a mental model of when you are touching each of these sections,
because that will affect how much Ethersync is tracking.

For brevity we'll just give some examples for each of them:
- If you edit a file, the working tree is affected, but none of the others (yet).
- The staging area will influence what you are committing.
    - `git status` gives you an insight into what is staged and what might have changed relative to the current HEAD.
    - `git diff` shows you *what* has changed in the working directory relative to the current HEAD.
- If you create a `git commit`, the Git directory stores it as part of the repository and also moves the HEAD.
- Using `git reset` with certain parameters, it's also possible to "just" move the HEAD without touching the working directory, which is something we will take advantage of below.

It's also good to understand that `git push` and `git fetch` (beware of `git pull`, though) are working on a whole different level: They synchronize a remote repository with your local one, without affecting the working tree or staging area.
The reason why `git pull` is different is, because it *does* change the working directory, as it's a shorthand for some combination of `git fetch` and `git merge`.

TODO: Explain upstream tracking branches here as well? As background to `@{u}` below.

### How Ethersync and Git interact

- Ethersync cares *only* about changes to the working directory of files that you [don't have open](file-ownership.md) in an editor.
- In reverse, any change to the `.git` directory and the staging area (which is in fact also tracked in the `.git` repository) is *ignored* by Ethersync
- From that it follows that Ethersync does not care about
    - commits you create,
    - files that you stage or unstage, or
    - changes to the HEAD you're making (as long as you don't affect the working tree).

Let's assume you are working on a feature. What is most interesting:

Ethersync does **not synchronize** the Git repository for you.
How do you make sure that all of you have the same Git state at some point,
when you're committing and maybe even change branches or checkout files in between?

Here are some things you might do, in increasing "order of complexity":
- Check what you have been doing so far with `git diff` / `git status`.
    - Impact on Ethersync: None, you can do it safely any time.
- Synchronize with a remote repository with `git push` and `git fetch`.
    - Impact on Ethersync: None, you can do it safely any time.
- Use `git add` and the like to stage changes.
    - Impact on Ethersync: No big impact, you can do it safely any time, but this won't be synchronized to your peer.
- Use `git commit` to, well, create a commit in the current branch.
    - Impact on Ethersync: No visible impact. A peer will not get notified of this in any way.
- Use `git reset --soft` or `git reset --mixed` to modify the staging area and the HEAD "manually".
    - Impact on Ethersync: No visible impact. A peer will not get notified of this in any way.
- Use `git restore`/`git checkout -- <pathspec>`/`git reset --hard HEAD` to undo your changes or get a different content of a file from the Git history.
    - Impact on Ethersync: These tools are changing the working directory, but do not move the HEAD. You should be okay and your peer is still fully in sync with you.
- Use `git switch`/`git checkout` to switch to a different branch or get a specific file state from history.
    - Impact on Ethersync: As you're changing the whole working directory, this will be tracked! A peer's working directory will get the same working directory state (assuming no open files), but their HEAD does not move in the same way.

### Recommended Workflow

To summarize all these bits and pieces into one pairing workflow that manages the Git related parts that Ethersync neglects.

(btw, phew, this was a lot of background!
As with other sections of this book, it should give you the knowledge and power to feel very comfortable using Ethersync.

But if you don't have advanced Git knowledge, working with Ethersync and Git is certainly still possible.
In this case, just make sure you stick to this "basic recipe" :) )

- When you start the daemon, make sure you're both starting on the same commit with a clean staging area.
- When you've finished coding something commit-worthy:
    - All peers should stop typing/editing files.
    - *One* person should create the commit.
- The committer then pushes the commit to a remote repository.
- Any other peer can then fetch the changes without applying them.
    - Note: The changes *are* already applied to the working tree, through Ethersync.
    - We assume you are working on a branch that has a track.
- Now each peer can update the HEAD. The easiest way is if you have a tracking upstream branch:
    - `git reset --mixed @{u}` moves your HEAD to the same commit as the committer.
    - Use `git status`/`git diff` to double check that all of you have the same diff now.

### Other workflows

Things you could do, that we have not tested thoroughly:
- Hop around different branches and states of the Git repository.
- Use git pull to get the latest changes.

When doing any of this, we recommend to close all connected editors for a smooth synchronization (because of [ownership](file-ownership.md)).

## When Note Taking

In the note taking use case it's much simpler, but there's also a possible way to integrate and take advantage of it.

In this use case the assumption is that everyone has their own local git, which is not synchronized with anyone else.
You can then use it, to track which parts have been changed by others, for example while you were offline.

Let's say you have initially added and committed all notes.
- Whenever you are reconnecting to the cloud peer and are getting some changes, you can revise them by looking at the git diff.
- Then you can add and commit them with an unimportant commit message to set a "savepoint" for next time

It's also a nice little back-up in case anything goes wrong with they sync. Which might happen given that this is very new and bleeding edge software, be it through bugs or misunderstandings.

## Ignoring `.ethersync` directories

In Ethersync-enabled projects, you will have a directory called `.ethersync`.

If you always want to ignore these directories, you can add it to your global `.gitignore` file like this:

```bash
mkdir -p ~/.config/git/
echo ".ethersync/" >> ~/.config/git/ignore
```
