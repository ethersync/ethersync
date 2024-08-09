local changetracker = require("changetracker")
local cursor = require("cursor")

-- JSON-RPC connection.
local client

-- Registry of the files that are synced.
local files = {}

-- Pulled out as a method in case we want to add a new "offline simulation" later.
local function send_notification(method, params)
    client.notify(method, params)
end

local function send_request(method, params, result_callback, err_callback)
    err_callback = err_callback or function() end
    result_callback = result_callback or function() end

    client.request(method, params, function(err, result)
        if err then
            local error_msg = "[ethersync] Error for '" .. method .. "': " .. err.message
            if err.data and err.data ~= "" then
                error_msg = error_msg .. " (" .. err.data .. ")"
            end
            vim.api.nvim_err_writeln(error_msg)
            err_callback(err)
        end
        if result then
            result_callback(result)
        end
    end)
end

-- Take an operation from the daemon and apply it to the editor.
local function process_operation_for_editor(method, parameters)
    if method == "edit" then
        local uri = parameters.uri
        -- TODO: Determine the proper filepath (relative to project dir).
        local filepath = vim.uri_to_fname(uri)
        local delta = parameters.delta.delta
        local the_editor_revision = parameters.delta.revision

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

-- Connect to the daemon.
local function connect()
    if client then
        client.terminate()
    end

    local params = { "client" }

    local socket_path = os.getenv("ETHERSYNC_SOCKET")
    if socket_path then
        table.insert(params, "--socket-path=" .. socket_path)
    end

    local dispatchers = {
        notification = function(method, notification_params)
            process_operation_for_editor(method, notification_params)
        end,
        on_error = function(code, ...)
            print("Ethersync client connection error: ", code, vim.inspect({ ... }))
        end,
        on_exit = function(...)
            print("Ethersync client connection exited: ", vim.inspect({ ... }))
        end,
    }

    if vim.version().api_level < 12 then
        -- In Vim 0.9, the API was to pass the command and its parameters as two arguments.
        ---@diagnostic disable-next-line: param-type-mismatch
        client = vim.lsp.rpc.start("ethersync", params, dispatchers)
    else
        -- While in Vim 0.10, it is combined into one table.
        local cmd = params
        table.insert(cmd, 1, "ethersync")
        client = vim.lsp.rpc.start(cmd, dispatchers)
    end

    print("Connected to Ethersync daemon!")
end

local function is_ethersync_enabled(filename)
    -- Recusively scan up directories. If we find an .ethersync directory on any level, return true.
    return vim.fs.root(filename, ".ethersync") ~= nil
end

local function track_edits(filename, uri)
    files[filename] = {
        -- Number of operations the daemon has made.
        daemon_revision = 0,
        -- Number of operations we have made.
        editor_revision = 0,
    }

    -- Vim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(0)) == 0 then
        vim.bo.eol = false
    end

    changetracker.track_changes(0, function(delta)
        files[filename].editor_revision = files[filename].editor_revision + 1

        local rev_delta = {
            delta = delta,
            revision = files[filename].daemon_revision,
        }

        local params = { uri = uri, delta = rev_delta }

        send_request("edit", params)
    end)
    cursor.track_cursor(0, function(ranges)
        local params = { uri = uri, ranges = ranges }
        -- Even though it's not "needed" we're sending requests in this case
        -- to ensure we're processing/seeing potential errors.
        send_request("cursor", params)
    end)
end

-- Forward buffer edits to daemon as well as subscribe to daemon events ("open").
local function on_buffer_open()
    local filename = vim.fn.expand("%:p")

    if not is_ethersync_enabled(filename) then
        return
    end

    if not client then
        connect()
    end

    local uri = "file://" .. filename
    send_request("open", { uri = uri }, function()
        track_edits(filename, uri)
    end)
end

local function on_buffer_close()
    local closed_file = vim.fn.expand("<afile>:p")

    if not is_ethersync_enabled(closed_file) then
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
    send_notification("close", { uri = uri })
end

local function print_info()
    if client then
        print("Connected to Ethersync daemon." .. "\n" .. cursor.list_cursors())
    else
        print("Not connected to Ethersync daemon.")
    end
end

vim.api.nvim_create_autocmd({ "BufRead", "BufNewFile" }, { callback = on_buffer_open })
vim.api.nvim_create_autocmd("BufUnload", { callback = on_buffer_close })

vim.api.nvim_create_user_command("EthersyncInfo", print_info, {})
vim.api.nvim_create_user_command("EthersyncJumpToCursor", cursor.jump_to_cursor, {})
