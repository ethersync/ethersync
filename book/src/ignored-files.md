<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Ignored files

Teamtype ignores

- `.teamtype` and everything in it,
- everything that Git would [ignore](https://git-scm.com/docs/gitignore), and
- version control directories including `.git`, `.jj`, `.bzr`, `.hg`, `.pijul`, and even `.svn`, and everything in them by default. The `--sync-vcs` flag enables sharing these directories, see [here](git-integration-synchronized.md) for details.

To prevent Teamtype from sharing files that contain sensitive information, like secrets, with your peers, add them to a `.gitignore` file.
