local changetracker = require("changetracker")

-- JSON-RPC connection.
local client

-- Currently we're only supporting editing *one* file. This string identifies, which one that is.
local theFile

-- Number of operations the daemon has made.
local daemonRevision = 0
-- Number of operations we have made.
local editorRevision = 0

local function sendNotification(method, params)
    client.notify(method, params)
end

-- Take an operation from the daemon and apply it to the editor.
local function processOperationForEditor(method, parameters)
    if method == "edit" then
        local _uri = parameters.uri --[[@diagnostic disable-line]]
        local delta = parameters.delta.delta
        local theEditorRevision = parameters.delta.revision

        if theEditorRevision == editorRevision then
            -- Find correct buffer to apply edits to.
            local bufnr = vim.uri_to_bufnr("file://" .. theFile)

            changetracker.applyDelta(bufnr, delta)

            daemonRevision = daemonRevision + 1
        else
            -- Operation is not up-to-date to our content, skip it!
            -- The daemon will send a transformed one later.
        end
    else
        print("Unknown method: " .. method)
    end
end

-- Send "open" message to daemon for this buffer.
local function openCurrentBuffer()
    local uri = "file://" .. theFile
    sendNotification("open", { uri = uri })
end

local function disconnect()
    if client then
        client.terminate()
        local buffer = vim.uri_to_bufnr("file://" .. theFile)
        vim.api.nvim_buf_set_option(buffer, "modifiable", false)
    end
end

-- Connect to the daemon.
local function connect(socket_path)
    disconnect()

    local params = { "client" }
    if socket_path then
        table.insert(params, "--socket-path=" .. socket_path)
    end
    client = vim.lsp.rpc.start("ethersync", params, {
        notification = function(method, notification_params)
            processOperationForEditor(method, notification_params)
        end,
        on_error = function(code, ...)
            print("Ethersync client connection error: ", code, vim.inspect({ ... }))
        end,
        on_exit = function(...)
            -- TODO: Is it a problem to do this in a schedule?
            vim.schedule(function()
                local bufnr = vim.uri_to_bufnr("file://" .. theFile)
                vim.api.nvim_buf_set_option(bufnr, "modifiable", false)
            end)

            print("Ethersync client connection exited: ", vim.inspect({ ... }))
            vim.defer_fn(connect, 1000)
        end,
    })
    if client then
        print("Connected to Ethersync daemon!")
        openCurrentBuffer()
        vim.bo.modifiable = true
        editorRevision = 0
        daemonRevision = 0
    else
        vim.defer_fn(connect, 1000)
    end
end

-- Forward buffer edits to daemon as well as subscribe to daemon events ("open").
function EthersyncOpenBuffer()
    if vim.fn.isdirectory(vim.fn.expand("%:p:h") .. "/.ethersync") ~= 1 then
        return
    end

    if not theFile then
        -- Only sync the *first* file loaded and nothing else.
        theFile = vim.fn.expand("%:p")
        vim.bo.modifiable = false
        connect()
        print("Ethersync activated for file " .. theFile)
    end

    if theFile ~= vim.fn.expand("%:p") then
        return
    end

    if not client then
        vim.bo.modifiable = false
    end

    -- Vim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(0)) == 0 then
        vim.bo.eol = false
    end

    changetracker.trackChanges(0, function(delta)
        editorRevision = editorRevision + 1

        local rev_delta = {
            delta = delta,
            revision = daemonRevision,
        }

        local uri = "file://" .. vim.api.nvim_buf_get_name(0)
        local params = { uri = uri, delta = rev_delta }

        sendNotification("edit", params)
    end)
end

function EthersyncCloseBuffer()
    local closedFile = vim.fn.expand("<afile>:p")
    if theFile ~= closedFile then
        return
    end
    -- TODO: Is the on_lines callback un-registered automatically when the buffer closes,
    -- or should we detach it ourselves?
    -- vim.api.nvim_buf_detach(0) isn't a thing. https://github.com/neovim/neovim/issues/17874
    -- It's not a high priority, as we can only generate edits when the buffer exists anyways.
    local uri = "file://" .. closedFile
    sendNotification("close", { uri = uri })
end

vim.api.nvim_create_autocmd({ "BufRead", "BufNewFile" }, { callback = EthersyncOpenBuffer })
vim.api.nvim_create_autocmd("BufUnload", { callback = EthersyncCloseBuffer })
vim.api.nvim_create_user_command("Etherbonk", function()
    connect("/tmp/etherbonk")
end, {})
