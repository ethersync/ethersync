<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

---
status: accepted
date: 2024-08-22
---
# How to prevent unauthorized access?

## Context and Problem Statement

In many situations, we don't want shared directories to be publicly readable & writable. How can we make sure that only the people we want can get access?

Our "attack surface" for a Teamtype daemon is connections from the Internet. We explicitly don't guard against misbehaving editor plugins, as they'd run on the local machine anyway (and as such could do damage more directly than going through the deamon, which is assumed to only run with user privileges).

We need two things:

* **Transport encryption**, so that nobody can spy on ongoing connections.
* An **authorization mechanism**, so that only certain people can start speaking the Automerge sync protocol with existing peers. Exactly those people should be able to read/change/delete all content in the shared directory.

The session lifetime is up to the users and can, in the ["note-taking use-case"](https://teamtype.github.io/teamtype/use-cases/shared-notes.html)
be for an indefinite amount of time.
A scenario where we're probably most vulnerable is when someone is leaving a "cloud peer" open and accessible by everyone.
We probably won't be able to solve this problem ourselves, but users might need to add other solutions, e.g. a VPN,
if they want to limit the risk.

## Decision Drivers

The solution should be:

* Simple to understand (no knowledge of public/private key systems required)
* Simple to use (we'd like it if peers can access a document if they know a common, secret passphrase)
* Rely on well-established, well-tested cryptography (that we didn't write ourselves)

## Considered Options

For transport encryption:

* TLS
* QUIC
* libp2p (transport-agnostic library)

For authorization:

* (for TLS) Pre-shared key initiation
* (for libp2p) "Positive list" of authorized peer IDs
* (for libp2p) Private network with pre-shared key
* (for all transports) Password-authenticated key agreement algorithm (like CPace)

## Decision Outcome

Chosen option: libp2p + private network with pre-shared key

We picked this solution because it requires the least amount of code/work from us, and gives us the properties we currently need.

Additional properties of our chosen solution:

- libp2p automatically gives us [*authentication* of peers through the Peer ID](https://docs.libp2p.io/concepts/security/security-considerations/#identity-and-trust)
    - Like with Signal, you have to verify the ID via an already trusted channel.
    - It's part of the multiaddr, so you can be sure there's no person-in-the-middle when given the multiaddr over a trusted channel.
- Access is irrevocable, peers get the entire document history on first sync.
- In order to kick someone out (prevent syncs from that point on), others have to agree on a new shared password.

## Pros and Cons of the Options

### Transport encryption

#### TLS

* Good, because it is very well-established
* Good, because it could be paired with pre-shared key initiation
* Neutral, because maybe users would need a little knowledge about public/private key systems
* Bad, because it locks us into a single transport (TCP)

#### QUIC

* Good, because it is (supposed to be) fast
* Good, because it has transport encryption built-in
* Bad, because it locks us into a single transport (UDP)

#### libp2p

* Good, because it is always transport encrypted by default
* Good, because it is transport agnostic
* Good, because it is very high-level and requires little code
* Good, because it has built-in support for making connections between local networks (like relays, AutoNAT and hole-punching)
* Good, because it easily gives "true" peer-to-peer behaviour – all peers can be dialed
* Bad, because it is internally the most complex solution

### Authorization

#### (for TLS) pre-shared key initiation

* Good, because it is very well-established, and only adds a single encryption layer as overhead

#### (for libp2p) "positive list" of authorized peer IDs

This could use the [libp2p_allow_block_list](https://docs.rs/libp2p-allow-block-list/latest/libp2p_allow_block_list/) crate.

* Good, because it adds fine-grained control (you can later revoke access for individual peers)
* Good, because even if the list of peer IDs leaks, attackers still don't have the private key to make a valid connection
* Bad, because it is more difficult to use (you have to send peer IDs around, and add them to the positive lists for each peer)

#### (for libp2p) private network with pre-shared key

This approach is specified [here](https://github.com/libp2p/specs/blob/master/pnet/Private-Networks-PSK-V1.md),
with a code snippet demonstrating its use [here](https://github.com/libp2p/rust-libp2p/discussions/5135#discussioncomment-8308069).

* Good, because it requires very little code
* Good, because it is simple to give access to a group of people (communicating the server's multiaddr + the secret over an established confidential channel)
* Bad, because we might not be able to use this approach with relays (to be investigated)
* Bad, because the PSK might be susceptible to brute-force attacks (especially when a session is running for longer)

#### (for all transports) password-authenticated key agreement

This is the approach used by [pcp](https://github.com/dennis-tra/pcp).

* Good, because it would be very transport agnostic
* Good, because it would work with relays and NAT techniques
* Bad, because we're not sure how to do that correctly/securely

## More Information

Another piece of our "security concept":

The daemon should only ever read and write in its base directory, not outside of it. We try to enforce this by routing all file I/O through a "sandbox" module, which takes a base directory as an additional argument, and tries to make sure all I/O stays inside of it. One exception is the UNIX socket file, which is written to `/tmp` right now.
