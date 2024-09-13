# Introduction

Have you used software like Google Docs, Etherpad or Hedgedoc? It's a very direct way to collaborate – multiple people can type into a document at the same time, and you can see each other's cursors.

Ethersync enables a workflow like that, but for **local text files, using your favorite text editors** like Neovim or VS Code!

> ⚠️ **Warning:**
>
> Ethersync is still in active development. At this point in time it is usable (we use it every day!) but has a lot of subtle things to know about and things that you might expect to work that do not yet work. Consider it as a proof of concept, and make sure to have backups.  
>
> A main reason we have written a lot of documentation is to give you the ability to learn about this system and with that allow you to use it smoothly despite the caveats.

## Current Features

- 👥 Real-time collaborative text editing
- 📍 See other people's cursors
- 🗃️ Work on entire projects
- 🛠️ Sync changes done by text editors and external tools
- ✒️ Local-first: You always have full access, even offline
- 🇳 Fully-featured Neovim plugin
- 🧩 Simple protocol for writing new editor plugins
- 🌐 Peer-to-peer connections, no need for a server
- 🔒 Encrypted connections secured by a shared password

## Planned features

- 🪟 VS Code plugin
- 🔄 Individual undo/redo (we probably won't work on this soon)

## Documentation overview

The main part of this documentation is aimed at users:

- [Getting started](getting-started.md) shows you how to install Ethersync, and how to make your first steps in it.
- [Concepts](concepts.md) goes into the fundamentals of how Ethersync operates, which is important for using it effectively.
- [Features](features.md) explains various things you can do with it (and some thing you can't do yet).
- [Ethersync in practice](in-practice.md) contains detailed advice for how to use Ethersync for certain workflows.
- [Related projects](related-projects.md) lists other software which have attempted to build similar systems.

There is also a section aimed at people who want to help improving Ethersync:

- [Writing new editor plugins](editor-plugin-dev-guide.md) specifies the protocol we use for communicating between the daemon and editors, and lists other things a plugin needs to do.
- [What to learn from us](learn-from-us.md) might be interesting to you if you want to build new software like this, especially if you find this after Ethersync has died. 💀
