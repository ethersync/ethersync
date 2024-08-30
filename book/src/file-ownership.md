# File ownership

When someone makes a change to a file, the daemons of connected peers will usually write that change directly to the disk.

However, once that file has been opened in an editor, that is undesirable – text editors are not happy if you change their files while they're running. So by opening a file in an editor with Ethersync plugin, that editor takes "ownership" of the file – the daemon will not write to them anymore. Instead, it will communicate changes to the editor plugin, which is then responsible for updating the editor buffer.

Once you close the file, the daemon will write the correct content to the file again. This means that, in an Ethersync-enabled directory, **saving files manually is not required, and doesn't have any meaning** – you can do it if you want, but your edits will be communicated to your peers immediately anyway.
