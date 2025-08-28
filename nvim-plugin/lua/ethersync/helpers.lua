-- SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local M = {}

function M.find_directory(filename, marker)
    -- Recusively scan up directories. If we find an .ethersync directory on any level, return its parent, and nil otherwise.
    if vim.version().api_level < 12 then
        -- In Neovim 0.9, do it manually.
        local path = filename
        while true do
            if vim.fn.isdirectory(path .. "/" .. marker) == 1 then
                return path
            end
            local parentPath = vim.fn.fnamemodify(path, ":h")
            if parentPath == path then
                -- We can't progress further like this.
                return nil
            else
                path = parentPath
            end
        end
    else
        -- In Neovim 0.10, this function is available.
        return vim.fs.root(filename, marker)
    end
end

return M