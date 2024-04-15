local utils = require("utils")
local sync = require("vim.lsp.sync")

-- Used to store the changedtick of the buffer when we make changes to it.
-- We do this to avoid infinite loops, where we make a change, which would
-- trigger normally an "on_bytes" event.
--
-- TODO: how big will this list get? should we optimize it?
local ignored_ticks = {}

local ns_id = vim.api.nvim_create_namespace("Ethersync")
local virtual_cursor

-- JSON-RPC connection.
local client

-- Toggle to simulate the editor going offline.
local online = false

-- Queues filled during simulated "offline" mode, and consumed when we go online again.
local opQueueForDaemon = {}
local opQueueForEditor = {}

-- Number of operations the daemon has made.
local daemonRevision = 0
-- Number of operations we have made.
local editorRevision = 0

-- Used to remember the previous content of the buffer, so that we can
-- calculate the difference between the previous and the current content.
local prev_lines

local function ignoreNextUpdate()
    local nextTick = vim.api.nvim_buf_get_changedtick(0)
    ignored_ticks[nextTick] = true
end

-- Creates a virtual cursor.
local function createCursor()
    local row = 0
    local col = 0
    virtual_cursor = vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
        hl_mode = "combine",
        hl_group = "TermCursor",
        end_col = col,
    })
end

-- Set the cursor position in the current buffer. If head and anchor are different,
-- a visual selection is created. head and anchor are in UTF-16 code units.
local function setCursor(head, anchor)
    if head == anchor then
        anchor = head + 1
    end

    if head > anchor then
        head, anchor = anchor, head
    end

    -- If the cursor is at the end of the buffer, don't show it.
    -- This is because otherwise, the calculation that follows (to find the location for head+1 would fail.
    -- TODO: Find a way to display the cursor nevertheless.
    if head == utils.contentOfCurrentBuffer() then
        return
    end

    local row, col = utils.indexToRowCol(head)
    local rowAnchor, colAnchor = utils.indexToRowCol(anchor)

    vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
        id = virtual_cursor,
        hl_mode = "combine",
        hl_group = "TermCursor",
        end_col = colAnchor,
        end_row = rowAnchor,
    })
end

-- Take an operation from the daemon and apply it to the editor.
local function processOperationForEditor(method, parameters)
    if method == "operation" then
        local theEditorRevision = tonumber(parameters[1])
        local changes = parameters[2]

        if theEditorRevision == editorRevision then
            local position = 0
            for _, change in ipairs(changes) do
                if type(change) == "number" then
                    if change > 0 then
                        position = position + change
                    elseif change < 0 then
                        ignoreNextUpdate()
                        utils.delete(position, -change)
                    else
                        -- Ignore.
                    end
                elseif type(change) == "string" then
                    ignoreNextUpdate()
                    utils.insert(position, change)
                end
            end
            -- log the operation to LOGFILE
            local file = io.open("/tmp/ethersync-vim-log", "a")
            file:write(vim.inspect(parameters) .. "\n")
            file:close()

            daemonRevision = daemonRevision + 1
        else
            -- Operation is not up-to-date to our content, skip it!
            -- The daemon will send a transformed one later.
            print(
                "Skipping operation, my editor revision is "
                    .. editorRevision
                    .. " but operation is for revision "
                    .. theEditorRevision
            )
        end
    else
        print("Unknown method: " .. method)
    end
end

-- Connect to the daemon.
local function connect(socket_path)
    params = { "client" }
    if socket_path then
        table.insert(params, "--socket-path=" .. socket_path)
    end
    client = vim.lsp.rpc.start("ethersync", params, {
        notification = function(method, params)
            if online then
                processOperationForEditor(method, params)
            else
                table.insert(opQueueForEditor, { method, params })
            end
        end,
    })
    online = true

    local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
    client.notify("open", { filename })
end

local function connect2()
    if client then
        client.terminate()
    end
    connect("/tmp/etherbonk")
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

-- Reset the state on editor side and re-open the current buffer
--
-- (this is to be called on buffer change, once we have the ability to detect that)
local function resetState()
    daemonRevision = 0
    editorRevision = 0
    opQueueForDaemon = {}
    opQueueForEditor = {}
    local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
    client.notify("open", { filename })
end

-- Initialization function.
function Ethersync()
    if vim.fn.isdirectory(vim.fn.expand("%:p:h") .. "/.ethersync") ~= 1 then
        return
    end

    print("Ethersync activated!")

    connect()

    createCursor()

    prev_lines = vim.api.nvim_buf_get_lines(0, 0, -1, true)

    vim.api.nvim_buf_attach(0, false, {
        on_lines = function(
            _the_literal_string_lines --[[@diagnostic disable-line]],
            _buffer_handle --[[@diagnostic disable-line]],
            changedtick,
            first_line,
            last_line,
            new_last_line
        )
            --client.notify("debug", { first_line = first_line, last_line = last_line, new_last_line = new_last_line })
            local curr_lines = vim.api.nvim_buf_get_lines(0, 0, -1, true)

            -- Did the change come from us? If so, ignore it.
            if ignored_ticks[changedtick] then
                ignored_ticks[changedtick] = nil
                prev_lines = curr_lines
                return
            end

            -- TODO: dirty fix that I don't understand any more
            table.insert(curr_lines, "")
            table.insert(prev_lines, "")
            table.insert(prev_lines, "")

            --client.notify("debug", { curr_lines = curr_lines, prev_lines = prev_lines })
            local diff = sync.compute_diff(prev_lines, curr_lines, first_line, last_line, new_last_line, "utf-8", "\n")
            --client.notify("debug", { diff = diff })

            local start_row = diff.range.start.line + 1
            local start_col = diff.range.start.character
            local end_row = diff.range["end"].line + 1
            local end_col = diff.range["end"].character

            --client.notify(
            --    "debug",
            --    { start_row = start_row, start_col = start_col, end_row = end_row, end_col = end_col }
            --)

            local range_start_char = utils.rowColToIndexInLines(start_row, start_col, prev_lines)
            local range_end_char = utils.rowColToIndexInLines(end_row, end_col, prev_lines)

            --client.notify("debug", { range_start_char = range_start_char, range_end_char = range_end_char })

            local deleted_chars = range_end_char - range_start_char
            if deleted_chars > 0 then
                editorRevision = editorRevision + 1
                if online then
                    client.notify("delete", { "", daemonRevision, range_start_char, deleted_chars })
                else
                    table.insert(
                        opQueueForDaemon,
                        { "delete", { "", daemonRevision, range_start_char, deleted_chars } }
                    )
                end
            end

            if #diff.text > 0 then
                editorRevision = editorRevision + 1
                if online then
                    client.notify("insert", { "", daemonRevision, range_start_char, diff.text })
                else
                    table.insert(opQueueForDaemon, { "insert", { "", daemonRevision, range_start_char, diff.text } })
                end
            end

            prev_lines = curr_lines
        end,
    })

    if vim.api.nvim_get_option_value("fixeol", { buf = 0 }) then
        if not vim.api.nvim_get_option_value("eol", { buf = 0 }) then
            utils.appendNewline()
            vim.api.nvim_set_option_value("eol", true, { buf = 0 })
        end
        vim.api.nvim_set_option_value("fixeol", false, { buf = 0 })
    end

    --vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
    --    callback = function()
    --        local row, col = unpack(vim.api.nvim_win_get_cursor(0))
    --        local head = utils.rowColToIndex(row, col)
    --        local headUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(head)

    --        if headUTF16CodeUnits == -1 then
    --            -- TODO what happens here?
    --            return
    --        end

    --        -- Is there a visual selection?
    --        local visualSelection = vim.fn.mode() == "v" or vim.fn.mode() == "V" or vim.fn.mode() == ""

    --        local anchorUTF16CodeUnits = headUTF16CodeUnits
    --        if visualSelection then
    --            -- Note: colV is the *byte* position, starting at *1*!
    --            local _, rowV, colV = unpack(vim.fn.getpos("v"))
    --            local anchor = utils.rowColToIndex(rowV, colV - 1)
    --            if head >= anchor then
    --                head = head + 1
    --            else
    --                anchor = anchor + 1
    --            end
    --            headUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(head)
    --            anchorUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(anchor)
    --        end
    --        local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
    --        client:notify("cursor", { filename, headUTF16CodeUnits, anchorUTF16CodeUnits })
    --    end,
    --})
end

function EthersyncClose()
    if vim.fn.isdirectory(vim.fn.expand("%:p:h") .. "/.ethersync") ~= 1 then
        return
    end
    local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
    client.notify("close", { filename })
end

-- When new buffer is loaded, run Ethersync automatically.
vim.api.nvim_exec(
    [[
augroup Ethersync
    autocmd!
    autocmd BufEnter * lua Ethersync()
    autocmd BufUnload * lua EthersyncClose()
augroup END
]],
    false
)

vim.api.nvim_create_user_command("Ethersync", Ethersync, {})

vim.api.nvim_create_user_command("EthersyncRunTests", utils.testAllUnits, {})
vim.api.nvim_create_user_command("EthersyncGoOffline", goOffline, {})
vim.api.nvim_create_user_command("EthersyncGoOnline", goOnline, {})
vim.api.nvim_create_user_command("EthersyncReload", resetState, {})
vim.api.nvim_create_user_command("Etherbonk", connect2, {})

-- TODO For debugging purposes. Remove before merging branch.
vim.api.nvim_create_user_command("EthersyncInsert", function()
    print(vim.fn.strchars(utils.contentOfCurrentBuffer()))
    local row, col = utils.indexToRowCol(2)
    print(row, col)
    utils.insert(2, "a")
end, {})
