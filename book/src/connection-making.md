<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Connection making

## Join codes

When you run `teamtype share`, you will get a short "join code" like `3-exhausted-bananas`. Another person can use it to connect to you! The code only works once. You can learn about the security properties in the [Magic Wormhole documentation](https://magic-wormhole.readthedocs.io/en/latest/welcome.html#safely).

## Secret addresses

Since version 0.7.0 Teamtype uses iroh for making a connection. To connect to another daemon, we're using a combination of the iroh [Node Identifier](https://www.iroh.computer/docs/concepts/endpoint#node-identifiers) and a secret key which, smashed together, which looks like `429e94...0e9819#32374e...4a6789`. We call this the node's *secret address*. Treat it like a password. After using a join code, the secret address is stored in your `.teamtype/config`.

## Peer to peer

You can directly connect across different local networks, even when each of you is behind a router. This way of connecting is more "ad hoc" and useful if you want to collaborate over a short period of time (as described in more detail in the [pair programming scenario](pair-programming.md)).

## Cloud peer

When you want to have an "always online" host, such that every user can connect to it at the time of their liking, let's say you're collaborating in a group on [taking notes](shared-notes.md).

Other systems solve this with a client-server architecture, where the server is always online, and the clients connect to it as needed.

But Teamtype is fundamentally peer-to-peer, so what we suggest to use is what the research group Ink & Switch call a ["cloud peer"](https://www.inkandswitch.com/local-first/): You run a Teamtype peer on a public server, and all users will then connect to that server.

This is only recommended for people who are comfortable setting up services on a server. But the nice part is that if someone did this for you, you can just connect to it not worrying about the nitty-gritty networking details.
