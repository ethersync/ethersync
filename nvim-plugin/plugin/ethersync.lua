-- SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local changetracker = require("ethersync.changetracker")
local cursor = require("ethersync.cursor")
local debug = require("ethersync.logging").debug
local helpers = require("ethersync.helpers")
local client = require("ethersync.client")

-- Registry of the files that are synced.
local files = {}

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

-- Forward buffer edits to daemon as well as subscribe to daemon events ("open").
local function on_buffer_open()
    -- TODO: Use <abuf> here?
    local filename = vim.fn.expand("%:p")
    debug("on_buffer_open: " .. filename)

    local directory = helpers.find_directory(filename, ".ethersync")
    if not directory then
        return
    end

    if not client.is_connected() then
        local success = client.connect(directory)
        if not success then
            return
        end
    end

    local uri = "file://" .. filename
    local buf = tonumber(vim.fn.expand("<abuf>"))

    -- Neovim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(buf)) == 0 then
        vim.bo.eol = false
    end

    local lines = changetracker.get_all_lines_respecting_eol(buf)
    local content = table.concat(lines, "\n")

    client.send_request("open", { uri = uri, content = content }, function()
        ensure_autoread_is_off()
        disable_writing()
        track_edits(filename, uri, lines)
    end)
end

local function on_buffer_new_file()
    -- Ensure that the file exists on disk before we "open" it in the daemon,
    -- to prevent a warning that the file has been created externally (W13).
    -- This resolves issue #92.
    vim.cmd("silent write")
    on_buffer_open()
end

local function on_buffer_close()
    local closed_file = vim.fn.expand("<afile>:p")

    if closed_file == "" then
        -- This is a temporary buffer without a name.
        return
    end

    debug("on_buffer_close: " .. closed_file)

    if not helpers.find_directory(closed_file, ".ethersync") then
        return
    end

    if not files[closed_file] then
        return
    end

    files[closed_file] = nil

    -- TODO: Is the on_lines callback un-registered automatically when the buffer closes,
    -- or should we detach it ourselves?
    -- vim.api.nvim_buf_detach(0) isn't a thing. https://github.com/neovim/neovim/issues/17874
    -- It's not a high priority, as we can only generate edits when the buffer exists anyways.

    local uri = "file://" .. closed_file
    client.send_notification("close", { uri = uri })
end

local function print_info()
    if client.is_connected() then
        print("Connected to Ethersync daemon." .. "\n" .. cursor.list_cursors())
    else
        print("Not connected to Ethersync daemon.")
    end
end

-- vim.api.nvim_create_autocmd({ "BufRead" }, { callback = on_buffer_open })
-- vim.api.nvim_create_autocmd({ "BufNewFile" }, { callback = on_buffer_new_file })
-- vim.api.nvim_create_autocmd("BufUnload", { callback = on_buffer_close })
--
-- -- This autocommand prevents that, when a file changes on disk while Neovim has the file open,
-- -- it should not attempt to reload it. Related to issue #176.
-- vim.api.nvim_create_autocmd("FileChangedShell", { callback = function() end })
--
-- vim.api.nvim_create_user_command("EthersyncInfo", print_info, {})
-- vim.api.nvim_create_user_command("EthersyncJumpToCursor", cursor.jump_to_cursor, {})
-- vim.api.nvim_create_user_command("EthersyncFollow", cursor.follow_cursor, {})
-- vim.api.nvim_create_user_command("EthersyncUnfollow", cursor.unfollow_cursor, {})
