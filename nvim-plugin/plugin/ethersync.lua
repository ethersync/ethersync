-- SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local ethersync = require("ethersync")

ethersync.config("ethersync", {
    cmd = { "ethersync", "client" },
    root_markers = ".ethersync",
})

ethersync.enable("ethersync")
