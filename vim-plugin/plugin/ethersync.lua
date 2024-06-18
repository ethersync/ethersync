local changetracker = require("changetracker")

-- JSON-RPC connection.
local client

-- Registry of the files that are synced.
local files = {}

-- Pulled out as a method in case we want to add a new "offline simulation" later.
local function sendNotification(method, params)
    client.notify(method, params)
end

-- Take an operation from the daemon and apply it to the editor.
local function processOperationForEditor(method, parameters)
    if method == "edit" then
        local uri = parameters.uri
        local filepath = vim.uri_to_fname(uri)
        local delta = parameters.delta.delta
        local theEditorRevision = parameters.delta.revision

        if theEditorRevision == files[filepath].editorRevision then
            -- Find correct buffer to apply edits to.
            local bufnr = vim.uri_to_bufnr(uri)

            changetracker.applyDelta(bufnr, delta)

            files[filepath].daemonRevision = files[filepath].daemonRevision + 1
        else
            -- Operation is not up-to-date to our content, ignore it!
            -- The daemon will send a transformed one later.
        end
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
            processOperationForEditor(method, notification_params)
        end,
        on_error = function(code, ...)
            print("Ethersync client connection error: ", code, vim.inspect({ ... }))
        end,
        on_exit = function(...)
            -- TODO: Is it a problem to do this in a schedule?
            vim.schedule(function()
                -- TODO: why did we have this?
                --local bufnr = vim.uri_to_bufnr("file://" .. theFile)
            end)

            print("Ethersync client connection exited: ", vim.inspect({ ... }))
        end,
    }

    if vim.version().api_level < 12 then
        -- In Vim 0.9, the API was to pass the command and its parameters as two arguments.
        client = vim.lsp.rpc.start("ethersync", params, dispatchers)
    else
        -- While in Vim 0.10, it is combined into one table.
        local cmd = params
        table.insert(cmd, 1, "ethersync")
        client = vim.lsp.rpc.start(cmd, dispatchers)
    end

    print("Connected to Ethersync daemon!")
end

local function IsEthersyncEnabled(filename)
    -- Recusively scan up directories. If we find an .ethersync directory on any level, return true.
    return vim.fs.root(filename, ".ethersync") ~= nil
end

-- Forward buffer edits to daemon as well as subscribe to daemon events ("open").
function EthersyncOpenBuffer()
    local filename = vim.fn.expand("%:p")

    if not IsEthersyncEnabled(filename) then
        return
    end

    if not client then
        connect()
    end

    files[filename] = {
        -- Number of operations the daemon has made.
        daemonRevision = 0,
        -- Number of operations we have made.
        editorRevision = 0,
    }

    local uri = "file://" .. filename
    sendNotification("open", { uri = uri })

    -- Vim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(0)) == 0 then
        vim.bo.eol = false
    end

    changetracker.trackChanges(0, function(delta)
        files[filename].editorRevision = files[filename].editorRevision + 1

        local rev_delta = {
            delta = delta,
            revision = files[filename].daemonRevision,
        }

        local params = { uri = uri, delta = rev_delta }

        sendNotification("edit", params)
    end)
end

function EthersyncCloseBuffer()
    local closedFile = vim.fn.expand("<afile>:p")

    if not IsEthersyncEnabled(closedFile) then
        return
    end

    if not files[closedFile] then
        return
    end

    files[closedFile] = nil

    -- TODO: Is the on_lines callback un-registered automatically when the buffer closes,
    -- or should we detach it ourselves?
    -- vim.api.nvim_buf_detach(0) isn't a thing. https://github.com/neovim/neovim/issues/17874
    -- It's not a high priority, as we can only generate edits when the buffer exists anyways.

    local uri = "file://" .. closedFile
    sendNotification("close", { uri = uri })
end

function EthersyncInfo()
    if client then
        print("Connected to Ethersync daemon!")
    else
        print("Not connected to Ethersync daemon.")
    end
end

vim.api.nvim_create_autocmd({ "BufRead", "BufNewFile" }, { callback = EthersyncOpenBuffer })
vim.api.nvim_create_autocmd("BufUnload", { callback = EthersyncCloseBuffer })

vim.api.nvim_create_user_command("EthersyncInfo", EthersyncInfo, {})
