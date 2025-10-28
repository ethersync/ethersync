<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Local first

A core idea of Teamtype is that working collaboratively on files is that everyone will have a copy on their computer even if they stop working together.

After you've initially synced with someone, your copy of the shared directory is fully independent from your peer. You can make changes to it, even when you don't have an Internet connection, and once you connect again, the daemons will sync in a more or less reasonable way. We can do this thanks to the magic of [CRDTs](https://en.wikipedia.org/wiki/Conflict-free_replicated_data_type) and the [Automerge](https://automerge.org) library.

See [offline support](offline-support.md) to learn more about how that works in practice.
