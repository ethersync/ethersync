# File Events

> 🚨 **Warning**
>
> The features described here will only work after [PR #133](https://github.com/ethersync/ethersync/pull/133) is merged.

Besides syncing what is changed using text editors, Ethersync also syncs changes made to the files with other tools. We use a file watcher to do that.

## New files

If you create a file in the project directory (or if you open one that doesn't exist yet), it will also appear in the directory of connected peers.

Example: `echo hello > new_file`

## Changing files

If you make changes to a file, the daemon will calculate a diff compared to the previous content, and send that to other peers as an edit.

Note: Edits will only be picked up if the file is not currently opened in an editor, because of [ownership](file-ownership.md).

Example: `echo new stuff >> new_file`

## Deleting files

If you delete a file, it will also disappear for other peers.

Example: `rm new_file`
