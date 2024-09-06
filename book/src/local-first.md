# Local first

After you've initially synced with someone, your copy of the shared directory is fully independent from your peer. You can make changes to it, even when you don't have an Internet connection, and once you connect again, the daemons will sync in a more or less reasonable way. We can do this thanks to the magic of [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) and the [Automerge](https://automerge.org) library.


## Behind the scenes

A CRDT is some kind of database that every peer maintains on their own, but when peers connect to each other, they will synchronize each other on their content.

The way Ethersync does this is by storing the CRDT in a file in your project. You can find it at `.ethersync/doc`. This file contains the edit history of all the files by each of the peers.

Sometimes it can make sense to delete this file to start with a clean state. This will especially become relevant in the case of switching to a different peer for example in a [pair-programming](pair-programming.md#reconnect-later) setup.
