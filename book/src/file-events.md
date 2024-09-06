# File Events

Ethersync currently only syncs changes done to the filesystem in very specific cases! We might improve this in the future, but for now, it's important to know which changes are sent to your peers:

## Creating files

### Changes that are synchronized:

- Opening a new file with an Ethersync-enabled text editor (this will create the file in the directory of connected peers).

    Example: `nvim new_file`

### Changes that are *not* synchronized:

- Creating a file directly on the file system.

    Example: `touch new_file`

- Copying in a file from outside the project.

    Example: `cp ../somewhere/else/file .`

## Changing files

### Changes that are synchronized:

- Editing a file in an Ethersync-enabled text editor.

    Example: `nvim existing_file`

### Changes that are *not* synchronized:

- Changing files with external tools. Examples:

    - `echo new stuff >> file`
    - `sort -o file file` (sorting a file in place)
    - `git restore file`

## Deleting files

This is an exception to the above: We actually watch the file system for deletion events, and transfer them over to other peers:

### Changes that are synchronized:

- Deleting a file directly from the file system!

    Example: `rm new_file`
