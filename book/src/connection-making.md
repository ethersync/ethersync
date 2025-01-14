<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Connection making

In order to make sense of how Ethersync daemons connect to each other a little bit of Networking background (IP addresses, TCP ports) is helpful. You should still be able to get going within one local network (such as two computers in the same Wi-Fi) by just copy pasting things, but connecting to other peers over the internet might currently require some configurations. We're aiming to give you the right keywords to look for in case you've not encountered that yet.

## Multiaddress

Ethersync uses libp2p for making a connection. To identify another daemon we're using a connection address like `/ip4/192.168.23.42/tcp/58063/p2p/12D3KooWPNj7mom3X2D6NiSyxbFa5hHfzxDFP98ZL52yYnkEVmDv`. This is what libp2p calls a [multiaddress](https://docs.libp2p.io/concepts/fundamentals/addressing/) – it contains your IP address, the TCP port, and a "peer ID" (which is used by connecting peers to make sure that they're actually connecting to the correct peer, and not to a "man in the middle").

## Port

By default Ethersync selects a random private port, but in this case you're trying to set up port forwarding or a cloud peer, it's probably helpful to fix the port for the hosting peers. This can be done through the `--peer` option or the configuration file as explained [here](configuration.md).

## Peer to peer

If you want to connect across different local networks where each of you is behind a router. This way of connecting is more "ad hoc" and useful if you want to collaborate over a short period of time (as described in more detail in the [pair programming scenario](pair-programming.md)).

You need to enable [*port forwarding*](https://en.wikipedia.org/wiki/Port_forwarding) on your router. Specifically, the hosting peer needs to configure their router in such a way that it forwards incoming connections on the port you're using with Ethersync to their local machine. Also the port might be blocked by a network firewall.

## Cloud peer

When you want to have an "always online" host, such that every user can connect to it at the time of their liking, let's say you're collaborating in a group on [taking notes](shared-notes.md).

Other systems solve this with a client-server architecture, where the server is always online, and the clients connect to it as needed.

But Ethersync is fundamentally peer-to-peer, so what we suggest to use is what the research group Ink & Switch call a ["cloud peer"](https://www.inkandswitch.com/local-first/): You run an Ethersync peer on a public server, and all users will then connect to that server.

This is only recommended for people who are comfortable setting up services on a server. But the nice part is that if someone did this for you, you can just connect to it not worrying about the nitty-gritty networking details.
