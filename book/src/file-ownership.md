# File ownership

Ethersync synchronizes edits immediately to each peer. Sometimes the peer has the file already open in an editor, sometimes not. In order to deal with different situations, we are using a concept called "ownership". Either the daemon or the editor can have it, it's like a token who is allowed to change the file.

## Daemon has ownership

The daemon has ownership of a file if it *is not open in an editor on that computer*. In this case the daemon is allowed to write the changes some other connected daemon makes to a file directly to the disk.

## Editor has ownership

However, once that file has been opened in an editor, that is undesirable – text editors are not happy if you change their files while they're running. So by opening a file in an editor with Ethersync plugin, that editor takes "ownership" of the file – the daemon will not write to them anymore. Instead, it will communicate changes to the editor plugin, which is then responsible for updating the editor buffer.

Once you close the file, the daemon will write the correct content to the file again. This means that, in an Ethersync-enabled directory, saving files manually is not required – you can do it if you want, but your edits will be communicated to your peers immediately anyway.

This is true as long as the daemon is running, so in case you're wrapping up your session, always make sure that you close all editors first and then the daemon, otherwise you might risk accidentally losing some of the edits to your buffer.

### Opening the same file with multiple editors

As you have seen in the ["first steps"](quickstart.md) section it is possible to open the same file in two editors and get the changes in both places. But keep in mind that our ownership detection is rather "basic" and does not have the ability to differentiate between those two editors. If you are opening the file with the second editor it's loading the content from disc, which might be out of date if you're not careful, leading to an out-of-sync state.

In short: We are not recommending this feature for the day to day use. It mainly exists to prevent crashes and allow debugging while testing.
