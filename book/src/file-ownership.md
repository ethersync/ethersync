# File ownership

Ethersync synchronizes edits immediately to each peer. Sometimes the peer has the file already open, sometimes not. In order to deal with different situations, we are using a concept called "ownership". Either the daemon or the editor can have it, it's like a token who is allowed to change the file.

## Daemon has ownership

The daemon has ownership if *no editor is connected to it*. In this case the daemon is allowed to write the changes some other connected daemon makes to a file directly to the disk.

## Editor has ownership

However, once that file has been opened in an editor, that is undesirable – text editors are not happy if you change their files while they're running. So by opening a file in an editor with Ethersync plugin, that editor takes "ownership" of the file – the daemon will not write to them anymore. Instead, it will communicate changes to the editor plugin, which is then responsible for updating the editor buffer.

Once you close the file, the daemon will write the correct content to the file again. This means that, in an Ethersync-enabled directory, saving files manually is not required – you can do it if you want, but your edits will be communicated to your peers immediately anyway.

This is true as long as the daemon is running, so in case you're wrapping up your session, always make sure that you close all editors first and then the daemon, otherwise you might risk accidentally losing some of the edits to your buffer.
