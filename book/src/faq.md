# FAQ

## What does "local-first" mean?

After you've initially synced with someone, your copy of the shared directory is fully independent from your peer. You can make changes to it, even when you don't have an Internet connection, and once you connect again, the daemons will sync in a more or less reasonable way. We can do this thanks to the magic of [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) and the [Automerge](https://automerge.org) library.

## What do you mean by "more or less reasonable" syncing?

The syncing will not always give 100% semantically correct results:

- When two people create a file with the same name at the same time, one of the two copies will win, and the other one will be overwritten. The daemon's log will tell you which copy won. We're planning to give you more choices or make a backup.
- When two people edit the same place of a source code, version control software like Git would show this as a "conflict", and ask you to resolve it manually.
Ethersync, on the other hand, allows the changes to smoothly integrate. The result is like the combination of their insertions and deletions. So the result will not necessarily compile.

However, the syncing should always guarantee that all peers have the same directory content.

## Can I make changes to a shared directory while the daemon isn't running?

Yes. When you start the daemon the next time, it will compare its persisted state to the actual disk content, calculate a diff, and bring the persisted state up to date. This often will be sufficient; but letting the daemon run and actually tracking the changes as you type them will sometimes lead to a more fine-grained, better syncing result.

## Can I edit a file with tools that don't have Ethersync support?

Yes, changes you make will be shared. However, there are fewer "correctness guarantees", especially if you make many edits in rapid progression.

You can also open a file in an editor without Ethersync plugin – if you change a file, and then save it, the edits will be shared. But if someone else has made an edit in the meantime, that edit will currently get lost.

## Can I open the same file in multiple editors at once?

[Not yet.](https://github.com/ethersync/ethersync/issues/63)

## Can one daemon share multiple directories at the same time?

[Not yet.](https://github.com/ethersync/ethersync/issues/134)
