<!--
SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>

SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Configuration

You can put the following options into a configuration file at `.ethersync/config`:

```ini
peer = <secret_address>
emit_join_code = <true/false>
emit_secret_address = <true/false>
```

After a successful `ethersync join`, the peer's secret address is automatically stored in your `.ethersync/config`.
In the future, you can then use `ethersync join` without a join code to reconnect to the same peer.