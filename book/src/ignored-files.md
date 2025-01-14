<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Ignored files

Some files and directories are ignored by default. Also you have the option to specify files that should be ignored.
Files that might contain sensitive information, like secrets, that should not be shared with your peers. Also Ethersync doesn't handle binary files, so maybe it makes sense to exclude them too.

Ethersync
- ignores `.git` and everything in it.
- ignores `.ethersync` and everything in it.
- it respects everything that Git would [ignore](https://git-scm.com/docs/gitignore).
