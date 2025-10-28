<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# File Events

Teamtype tries to sync not only file changes done by supported editors, but also by external tools.

> ⚠️ **Warning:**
>
> When one peer edits a file from an editor, and another peer changes it with an external tool at the same time, the latter change might get lost.
> This is a restriction that seems hard to avoid. If you want to make sure changes by external tools are recorded correctly, do them while the daemon is not running, and make use of Teamtype's [offline support](offline-support.md).

## Creating files

- Opening a new file with an Teamtype-enabled text editor (this will create the file in the directory of connected peers).

    Example: `nvim new_file`

- Creating a file directly on the file system.

    Example: `touch new_file`

- Copying in a file from outside the project.

    Example: `cp ../somewhere/else/file .`

## Changing files

- Editing a file in an Teamtype-enabled text editor.

    Example: `nvim existing_file`

- Changing files with external tools. Examples:

    - `echo new stuff >> file`
    - `sort -o file file` (sorting a file in place)
    - `git restore file`

## Deleting files

- Deleting a file directly from the file system.

    Example: `rm new_file`

- Moving a file out of the project.

    Example: `mv file ../somwhere/else/`
