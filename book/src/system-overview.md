# System overview

At this point in time Ethersync is a piece of software that is somewhat usable but
has a lot of traps and things that you might expect to work that do not yet work.
Consider it as a proof of concept that can be used in production if you're careful.
For details on the workarounds we have a [whole section](workarounds.md),
that we recommend to check out, once you're comfortable with the basics.

Why are we telling you this? To motivate you to learn some of the "behind the scenes"
such that you do know what to expect and how to deal with it.

In order to understand how Ethersync works, let's consider what problems it is solving and then which components are involved in achieving this.

Ethersync is for a real-time local-first collaboration on text files, where
- real-time means that edits and cursor movements should appear immediately while you are in a connection with your peer
- local-first means that it's also possible to continue working on the project while you're (temporarily) offline
- and the collaboration is restricted to text-files only (unfortunately no docx, xlsx, images, etc.)

When two or more people are collaborating on text, we are communicating each individual change to the other peers
as soon as it's possible.

There are two ways to change the file
- through an editor with an Ethersync plugin
- with any tool directly in place/on disc

This change gets recorded by the daemon. If you change it through the editor,
the daemon is able to track every single character edit, and has potentially an easier time to resolve conflicts.
If you just replace the file, the daemon will infer what edits you were doing, but in a much coarser way.

The daemon then communicates the changes with other connected daemons.
If conflicts arise, because two edits happened at the same time, they will be resolved by the daemon automatically.

What if we're currently offline?
Then the daemon records the change locally and will communicate it to the other peers later.

Involved components:
- editor
- editor plugin (which btw also uses the ethersync software)
- daemon

The daemon is:
- the collector of changes
- the keeper of "the truth"
- the resolver of conflicts

The editor plugin is:
- the communicator of edits
- knows about cursor positions (and lets you jump to them)

The editor then displays the latest known state and visible cursor positions.

## The project

When collaborating with your peers we assume that you are working on a set of files which are co-located.

We call this location the "project".
You can compare it, if you're familiar with that, with a git repository.

This project, this set of files, is identified by one directory.
The tracking of, and communication about changes happens only inside the realm of that directory
and whatever it contains recursively (which means it includes sub-directories and the files therein).

As of this version you will need to start one daemon *per project*.
When you start the daemon, you have the option to provide the directory as an optional parameter:

    ethersync daemon [OPTIONS] [DIRECTORY]

If you leave it out, the current directory is selected.

### Ignored files

While we stated above that everything in the project is synchronized, this was not fully correct.
You have the option to specify files that should be ignored.
Files that might contain sensitive information, like secrets, that should not be shared with your peers.

TODO: describe the basic ways to ignore files.
