-- SPDX-FileCopyrightText: 2025 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2025 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local changetracker = require("ethersync.changetracker")
local connection = require("ethersync.connection")
local cursor = require("ethersync.cursor")
local debug = require("ethersync.logging").debug
local helpers = require("ethersync.helpers")

local M = {}

-- Registry of possible configurations.
local configurations = {}

-- The active clients. Each client has:
-- name: The name of the configuration (available options: the keys in the above dictionary)
-- files: a registry of files that are synced
-- root_dir: the root directory
-- connection: a JSON-RPC connection
-- buffers: list of attached buffers
local clients = {}

function M.config(name, cfg)
    if cfg.cmd == nil then
        error("Configuration '" .. name .. "' should have a `cmd` to specify which collaboration tool to launch.")
    end

    if cfg.root_markers == nil and cfg.root_dir == nil then
        error(
            "Configuration '"
                .. name
                .. "' should either have a `root_markers` or a `root_dir` to determine launch condition."
        )
    end
    configurations[name] = { cfg = cfg, enabled = false }
end

function M.enable(name, enable)
    if enable == nil then
        enable = true
    end

    configurations[name].enabled = enable
end

function M.status()
    if #clients == 0 then
        return ""
    end

    local result = "Teamtyping"

    local n = cursor.number_of_cursors()
    if n > 0 then
        result = result .. " with " .. cursor.short_cursor_description()
    end

    return result
end

-- Take an operation from the daemon and apply it to the editor.
local function process_operation_for_editor(client, method, parameters)
    if method == "edit" then
        local uri = parameters.uri
        -- TODO: Determine the proper filepath (relative to project dir).
        local filepath = vim.uri_to_fname(uri)
        local delta = parameters.delta
        local the_editor_revision = parameters.revision

        -- Check if operation is up-to-date to our content.
        -- If it's not, ignore it! The daemon will send a transformed one later.
        if the_editor_revision == client.files[filepath].editor_revision then
            -- Find correct buffer to apply edits to.
            local bufnr = vim.uri_to_bufnr(uri)

            changetracker.apply_delta(bufnr, delta)

            client.files[filepath].daemon_revision = client.files[filepath].daemon_revision + 1
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

local function track_edits(client, filename, uri, initial_lines)
    client.files[filename] = {
        -- Number of operations the daemon has made.
        daemon_revision = 0,
        -- Number of operations we have made.
        editor_revision = 0,
    }

    local bufnr = vim.uri_to_bufnr(uri)

    changetracker.track_changes(bufnr, initial_lines, function(delta)
        client.files[filename].editor_revision = client.files[filename].editor_revision + 1

        local params = { uri = uri, delta = delta, revision = client.files[filename].daemon_revision }

        client.connection:send_request("edit", params)
    end)
    cursor.track_cursor(bufnr, function(ranges)
        local params = { uri = uri, ranges = ranges }
        -- Even though it's not "needed" we're sending requests in this case
        -- to ensure we're processing/seeing potential errors.
        client.connection:send_request("cursor", params)
    end)
end

local function find_or_create_client(config_name, root_dir)
    -- We re-use connections for configs with the same name and root_dir.
    for _, client in ipairs(clients) do
        if client.name == config_name and client.root_dir == root_dir then
            return client
        end
    end

    -- No reusable connection? Let's create a new one.
    local client = {
        name = config_name,
        root_dir = root_dir,
        files = {},
        buffers = {},
        connection = nil,
    }
    local the_connection = connection.connect(configurations[config_name].cfg.cmd, root_dir, function(m, p)
        process_operation_for_editor(client, m, p)
    end)
    client.connection = the_connection
    table.insert(clients, client)

    return client
end

local function activate_config_for_buffer(config_name, buf_nr, root_dir)
    local client = find_or_create_client(config_name, root_dir)
    table.insert(client.buffers, buf_nr)

    local filename = vim.api.nvim_buf_get_name(buf_nr)
    local uri = vim.uri_from_bufnr(buf_nr)

    -- Neovim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(buf_nr)) == 0 then
        vim.bo.eol = false
    end

    local lines = changetracker.get_all_lines_respecting_eol(buf_nr)
    local content = table.concat(lines, "\n")

    -- Ensure that the file exists on disk before we "open" it in the daemon,
    -- to prevent a warning that the file has been created externally (W13).
    -- This resolves issue #92.
    if string.find(uri, "file://") == 1 and not vim.fn.filereadable(filename) then
        vim.cmd("silent write")
    end

    client.connection:send_request("open", { uri = uri, content = content }, function()
        debug("Tracking Edits")
        ensure_autoread_is_off()
        disable_writing()
        track_edits(client, filename, uri, lines)
    end)
end

local function on_buffer_open()
    local buf_nr = tonumber(vim.fn.expand("<abuf>"))
    local buf_name = vim.api.nvim_buf_get_name(buf_nr)

    for name, server in pairs(configurations) do
        if server.enabled then
            if server.cfg.root_dir then
                server.cfg.root_dir(buf_nr, function(root_dir)
                    activate_config_for_buffer(name, buf_nr, root_dir)
                end)
            elseif server.cfg.root_markers then
                local uri = vim.uri_from_bufnr(buf_nr)
                if string.find(uri, "file://") == 1 then
                    local root_dir = helpers.find_directory(buf_name, server.cfg.root_markers)
                    if root_dir then
                        activate_config_for_buffer(name, buf_nr, root_dir)
                    end
                else
                    debug(
                        "Did not activate '"
                            .. name
                            .. "' configuration for non-file buffer despite having a `root_markers` configured."
                    )
                end
            end
        end
    end
end

local function on_buffer_close()
    local buf_nr = tonumber(vim.fn.expand("<abuf>"))
    local closed_file = vim.fn.expand("<afile>:p")

    if closed_file == "" then
        -- This is a temporary buffer without a name.
        return
    end

    debug("on_buffer_close: " .. closed_file)

    -- Find the correct client, and remove this buffer from it.
    for _, client in ipairs(clients) do
        for j, buffer in ipairs(client.buffers) do
            if buffer == buf_nr then
                client.files[closed_file] = nil

                local uri = vim.uri_from_bufnr(buf_nr)
                client.connection:send_notification("close", { uri = uri })
                table.remove(client.buffers, j)
            end
        end
    end
end

local function print_info()
    if #clients == 0 then
        print("Not connected to any Ethersync daemon.")
        return
    end

    local result = "Clients:\n\n"

    for _, client in ipairs(clients) do
        result = result .. "\"" .. client.name .. "\" in '" .. client.root_dir .. "'\n"
    end

    result = result .. "\nCursors:\n\n" .. cursor.list_cursors()

    print(result)
end

local function activate_plugin()
    vim.api.nvim_create_autocmd({ "BufRead" }, { callback = on_buffer_open })
    vim.api.nvim_create_autocmd({ "BufNewFile" }, { callback = on_buffer_open })
    vim.api.nvim_create_autocmd("BufUnload", { callback = on_buffer_close })

    -- This autocommand prevents that, when a file changes on disk while Neovim has the file open,
    -- it should not attempt to reload it. Related to issue #176.
    vim.api.nvim_create_autocmd("FileChangedShell", { callback = function() end })

    vim.api.nvim_create_user_command("EthersyncInfo", print_info, {})
    vim.api.nvim_create_user_command("EthersyncJumpToCursor", cursor.jump_to_cursor, {})
    vim.api.nvim_create_user_command("EthersyncFollow", cursor.follow_cursor, {})
    vim.api.nvim_create_user_command("EthersyncUnfollow", cursor.unfollow_cursor, {})
end

activate_plugin()

return M
