-- SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local changetracker = require("ethersync.changetracker")
local client = require("ethersync.client")
local cursor = require("ethersync.cursor")
local debug = require("ethersync.logging").debug
local helpers = require("ethersync.helpers")

local M = {}

-- Registry of possible configurations.
local configs = {}

-- Registry of the files that are synced.
local files = {}

function M.config(name, cfg)
    -- TODO: check here if valid?
    configs[name] = cfg
    debug(vim.inspect(configs))
end

function M.enable(name, enable)
    if enable == nil then
        enable = true
    end

    if enable then
    end
end

-- Take an operation from the daemon and apply it to the editor.
local function process_operation_for_editor(method, parameters)
    if method == "edit" then
        local uri = parameters.uri
        -- TODO: Determine the proper filepath (relative to project dir).
        local filepath = vim.uri_to_fname(uri)
        local delta = parameters.delta
        local the_editor_revision = parameters.revision

        -- Check if operation is up-to-date to our content.
        -- If it's not, ignore it! The daemon will send a transformed one later.
        if the_editor_revision == files[filepath].editor_revision then
            -- Find correct buffer to apply edits to.
            local bufnr = vim.uri_to_bufnr(uri)

            changetracker.apply_delta(bufnr, delta)

            files[filepath].daemon_revision = files[filepath].daemon_revision + 1
        end
    elseif method == "cursor" then
        cursor.set_cursor(parameters.uri, parameters.userid, parameters.name, parameters.ranges)
    else
        print("Unknown method: " .. method)
    end
end

-- Disabling 'autoread' prevents Neovim from reloading the file when it changes externally,
-- but only in the case where the buffer hasn't been modified in Neovim already.
-- For the conflicting case, we prevent a popup dialog by setting the FileChangedShell autocommand below.
local function ensure_autoread_is_off()
    if vim.o.autoread then
        vim.bo.autoread = false
    end
end

-- In ethersync-ed buffers, "writing" is no longer a concept. We also want to avoid error messages
-- when the file has changed on disk, so make all writing operations a no-op.
local function disable_writing()
    local buf = vim.api.nvim_get_current_buf()
    local autocmd_arg = {
        buffer = buf,
        callback = function()
            -- Trigger this autocommand so that plugins like autoformatters still work.
            vim.api.nvim_exec_autocmds("BufWritePre", {
                buffer = buf,
            })
        end,
    }
    vim.api.nvim_create_autocmd("BufWriteCmd", autocmd_arg)
    vim.api.nvim_create_autocmd("FileWriteCmd", autocmd_arg)
    vim.api.nvim_create_autocmd("FileAppendCmd", autocmd_arg)
end

local function track_edits(filename, uri, initial_lines)
    files[filename] = {
        -- Number of operations the daemon has made.
        daemon_revision = 0,
        -- Number of operations we have made.
        editor_revision = 0,
    }

    local bufnr = vim.uri_to_bufnr(uri)

    changetracker.track_changes(bufnr, initial_lines, function(delta)
        files[filename].editor_revision = files[filename].editor_revision + 1

        local params = { uri = uri, delta = delta, revision = files[filename].daemon_revision }

        client.send_request("edit", params)
    end)
    cursor.track_cursor(bufnr, function(ranges)
        local params = { uri = uri, ranges = ranges }
        -- Even though it's not "needed" we're sending requests in this case
        -- to ensure we're processing/seeing potential errors.
        client.send_request("cursor", params)
    end)
end

local function activate_config_for_buffer(config_name, buf_nr, root_dir)
    debug("activating!")
    if not client.is_connected() then
        local success = client.connect(root_dir, process_operation_for_editor)
        if not success then
            return
        end
    end

    local filename = vim.api.nvim_buf_get_name(buf_nr)
    local uri = "file://" .. filename

    -- Neovim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(buf_nr)) == 0 then
        vim.bo.eol = false
    end

    local lines = changetracker.get_all_lines_respecting_eol(buf_nr)
    local content = table.concat(lines, "\n")

    client.send_request("open", { uri = uri, content = content }, function()
        debug("Tracking Edits")
        ensure_autoread_is_off()
        disable_writing()
        track_edits(filename, uri, lines)
    end)
end

local function on_buffer_open()
    debug("buf open")
    local buf_nr = tonumber(vim.fn.expand("<abuf>"))
    local buf_name = vim.api.nvim_buf_get_name(buf_nr)

    debug(vim.inspect(configs))
    for _, config in pairs(configs) do
        -- TODO: What if buf_name is not a file name?
        local root_dir = helpers.find_directory(buf_name, config.root_markers)
        debug(root_dir)
        if root_dir then
            activate_config_for_buffer(config.name, buf_nr, root_dir)
        end
    end
end

local function activate_plugin()
    vim.api.nvim_create_autocmd({ "BufRead" }, { callback = on_buffer_open })
end

activate_plugin()

return M