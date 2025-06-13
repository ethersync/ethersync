<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Configuration

After a successful `ethersync join`, the peer's secret address is stored in your `.ethersync/config` in the following format:

```ini
peer = <node id>#<passphrase>
```
In the future, can then use `ethersync join` without a join code, to reconnect to the same peer.