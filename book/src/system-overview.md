# System overview

This section gives you some introduction what's going on "behind the scenes".

In order to understand how Ethersync works, let's consider what problems it is solving and then which components are involved in achieving this.

Ethersync is for a real-time local-first collaboration on text files, where
- real-time means that edits and cursor movements should appear immediately while you are in a connection with your peer
- local-first means that it's also possible to continue working on the project while you're (temporarily) offline
- and the collaboration is restricted to text-files only (unfortunately no docx, xlsx, images, etc.)

When two or more people are collaborating on text, we are communicating each individual change to the other peers
as soon as it's possible.

Ethersync picks up all changes to the file, if it's done with an editor that has the plugin installed.

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
Most files are synchronized, exceptions see [ignored files](ignored-files.md).

As of this version you will need to start one daemon *per project*.
When you start the daemon, you have the option to provide the directory as an optional parameter:

    ethersync daemon [OPTIONS] [DIRECTORY]

If you leave it out, the current directory is selected.
