<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Related Projects

There have been a number of attempts of enabling collaborative text editing! If you think a project is missing, feel free to [submit an issue or a PR](https://github.com/teamtype/teamtype) to add them to the list!

| | Open-source | Actively developed[^active] | Peer-to-peer | Local-first[^local-first] | Editor-agnostic |
|-|-|-|-|-|-|
| [Tandem](https://github.com/typeintandem/tandem)                                     | ✅ | ❌ | ✅ | ✅ | ✅ (Sublime, Neovim, Vim) |
| [Open Collab Tools](https://www.open-collab.tools)                                   | ✅ | ✅ | ❌ | ✅ | ✅ (VS Code, Eclipse Theia) |
| [crdt.el](https://github.com/zaeph/crdt.el)                                          | ✅ | ❌ | ✅ | ✅ | ❌ (Emacs) |
| [instant.nvim](https://github.com/jbyuki/instant.nvim)                               | ✅ | ❌ | ✅ | ✅ | ❌ (Neovim) |
| [Teletype](https://github.com/atom/teletype)                                         | ✅ | ❌ | ✅ | ✅ | ❌ (Atom) |
| [Etherpad](https://etherpad.org)                                                     | ✅ | ✅ | ❌ | ❌ | ❌ (Web) |
| [HedgeDoc](https://hedgedoc.org)                                                     | ✅ | ✅ | ❌ | ❌[^hedgedoc] | ❌ (Web) |
| [CryptPad](https://cryptpad.org)                                                     | ✅ | ✅ | ❌ | ❌ | ❌ (Web) |
| [Nextcloud Text](https://github.com/nextcloud/text)                                  | ✅ | ✅ | ❌ | ✅ | ❌ (Web) |
| [Rustpad](https://github.com/ekzhang/rustpad)                                        | ✅ | ✅ | ❌ | ❌ | ❌ (Web) |
| [SubEthaEdit](https://en.wikipedia.org/wiki/SubEthaEdit)                             | ✅ | ❌ | ✅ | ❌ | ❌ (Standalone) |
| [Gobby](https://en.wikipedia.org/wiki/Gobby)                                         | ✅ | ❌ | ✅ | ❌ | ❌ (Standalone) |
| [codemp](https://code.mp)                                                            | ❌[^server] | ✅ | ❌ | ✅ | ✅ (Neovim, VS Code, Sublime, IntelliJ) |
| [Floobits](https://github.com/Floobits)                                              | ❌[^server] | ❌ | ❌ | ❌ | ✅ (Sublime, Atom, Neovim, Vim, IntelliJ, Emacs) |
| [Google Docs](https://en.wikipedia.org/wiki/Google_Docs)                             | ❌ | ✅ | ❌ | ❌ | ❌ (Web) |
| [Visual Studio Live Share](https://visualstudio.microsoft.com/services/live-share/)  | ❌ | ✅ | ❌ | ❌ | ❌ (Visual Studio, VS Code) |
| [IntelliJ's Code With Me](https://www.jetbrains.com/help/idea/code-with-me.html)     | ❌ | ✅ | ❌ | ❌ | ❌ (IntelliJ) |

[^active]: As of September 2024
[^local-first]: This column indicates that the software uses CRDTs, we haven't checked for good offline support
[^hedgedoc]: Will use CRDTs starting with the (upcoming) version 2.0
[^server]: Open-source plugins, proprietary server
