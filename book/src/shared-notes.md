## Setting up Ethersync for a note-taking use-case

While in a "pair-programming" use case, all peers will be online at the same time, for shared notes, it is often desirable to allow peers to go offline, and other peers will still get their changes once they connect.

To enable that, our currently proposed solution is to set up a "cloud peer" – an Ethersync daemon running on a public server, which all users connect to. This resembles a server-client architecture, but all peers are essentially equal. Just the topology of the connections is star-shaped.
