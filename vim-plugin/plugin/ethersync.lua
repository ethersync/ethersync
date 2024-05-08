local utils = require("utils")
local changetracker = require("changetracker")

-- JSON-RPC connection.
local client

-- Toggle to simulate the editor going offline.
local online = false
-- Currently we're only supporting editing *one* file. This string identifies, which one that is.
local theFile

-- Queues filled during simulated "offline" mode, and consumed when we go online again.
local opQueueForDaemon = {}
local opQueueForEditor = {}

-- Number of operations the daemon has made.
local daemonRevision = 0
-- Number of operations we have made.
local editorRevision = 0

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

-- Reset the state on editor side.
local function resetState()
    daemonRevision = 0
    editorRevision = 0
    opQueueForDaemon = {}
    opQueueForEditor = {}
end

-- Connect to the daemon.
local function connect(socket_path)
    resetState()

    local params = { "client" }
    if socket_path then
        table.insert(params, "--socket-path=" .. socket_path)
    end
    client = vim.lsp.rpc.start("ethersync", params, {
        notification = function(method, notification_params)
            if online then
                processOperationForEditor(method, notification_params)
            else
                table.insert(opQueueForEditor, { method, notification_params })
            end
        end,
    })
    online = true
end

-- Send "open" message to daemon for this buffer.
local function openCurrentBuffer()
    local uri = "file://" .. theFile
    client.notify("open", { uri = uri })
end

local function connect2()
    if client then
        client.terminate()
    end
    connect("/tmp/etherbonk")
    openCurrentBuffer()
end

-- Simulate disconnecting from the daemon.
local function goOffline()
    online = false
end

-- Simulate connecting to the daemon again.
-- Apply both queues, then reset them.
local function goOnline()
    for _, op in ipairs(opQueueForDaemon) do
        local method = op[1]
        local params = op[2]
        client.notify(method, params)
    end

    for _, op in ipairs(opQueueForEditor) do
        local method = op[1]
        local params = op[2]
        processOperationForEditor(method, params)
    end

    opQueueForDaemon = {}
    opQueueForEditor = {}
    online = true
end

-- Forward buffer edits to daemon as well as subscribe to daemon events ("open").
function EthersyncOpenBuffer()
    if vim.fn.isdirectory(vim.fn.expand("%:p:h") .. "/.ethersync") ~= 1 then
        return
    end

    if not theFile then
        -- Only sync the *first* file loaded and nothing else.
        theFile = vim.fn.expand("%:p")
        connect()
        print("Ethersync activated for file " .. theFile)
    end

    if theFile ~= vim.fn.expand("%:p") then
        return
    end

    -- Vim enables eol for an empty file, but we do use this option values
    -- assuming there's a trailing newline iff eol is true.
    if vim.fn.getfsize(vim.api.nvim_buf_get_name(0)) == 0 then
        vim.bo.eol = false
    end

    openCurrentBuffer()

    changetracker.trackChanges(0, function(delta)
        editorRevision = editorRevision + 1

        local rev_delta = {
            delta = delta,
            revision = daemonRevision,
        }

        local uri = "file://" .. vim.api.nvim_buf_get_name(0)
        local params = { uri = uri, delta = rev_delta }

        if online then
            client.notify("edit", params)
        else
            table.insert(opQueueForDaemon, { "edit", params })
        end
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
    client.notify("close", { uri = uri })
end

vim.api.nvim_create_autocmd({ "BufRead", "BufNewFile" }, { callback = EthersyncOpenBuffer })
vim.api.nvim_create_autocmd("BufUnload", { callback = EthersyncCloseBuffer })

vim.api.nvim_create_user_command("EthersyncRunTests", utils.testAllUnits, {})
vim.api.nvim_create_user_command("EthersyncGoOffline", goOffline, {})
vim.api.nvim_create_user_command("EthersyncGoOnline", goOnline, {})
vim.api.nvim_create_user_command("EthersyncReload", resetState, {})
vim.api.nvim_create_user_command("Etherbonk", connect2, {})
