-- SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local teamtype = require("teamtype")

teamtype.config("teamtype", {
    cmd = { "teamtype", "client" },
    root_markers = ".teamtype",
})

teamtype.enable("teamtype")
