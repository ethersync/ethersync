---
status: accepted
date: 2024-03-14
---
# Ethersync UI Specification

This document contains our current idea of how the Commandline User Interface looks like.
(This document could later become the README. Note that the UI doesn't exist yet in this form.)

## Overview

A main inspiration for our UI is a tool called [`magic wormhole`](https://magic-wormhole.readthedocs.io/en/latest/welcome.html#example),
which uses a random but easily communicable keyword (number-word-word, e.g. 7-crossover-clockwork) to identify a certain transfer.

In our case the keyword identifies a shared project.
A shared project is an (ephemeral) session that is initiated from one end and can be joined by multiple other parties.

## Subcommands

- `share`
    - creates the session (and also ensures there's a daemon running to manage it)
    - will share all of the content of the directory it was initiated in
- `join`
    - allows to join a session
    - will synchronize the files from the session to the joining party
    - potentially creating new files and fetching latest changes.
- `status`
    - a generic command to give a user an overview about what's going on:
        - which shares are active
        - how many peers are connected

## Example

Host / initiator:
```
$ ethersync share
Initiating a new shared directory!
Sharing code: apple-camel-icecream
```

Joining peers:
```
$ ethersync join apple-camel-icecream
```

## Longer Running Session

If you want to ensure that the share is running for a longer time (allowing all participants to sign on and off), you'll have to initiate the session from a so called [cloud peer](https://www.inkandswitch.com/local-first/).
