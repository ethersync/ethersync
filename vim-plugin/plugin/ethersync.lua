local utils = require("utils")
local sync = require("vim.lsp.sync")

-- Used to store the changedtick of the buffer when we make changes to it.
-- We do this to avoid infinite loops, where we make a change, which would
-- trigger normally an "on_bytes" event.
--
-- TODO: how big will this list get? should we optimize it?
local ignored_ticks = {}
local ignore_edits = false

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

local function debug(tbl)
    if true then
        client.notify("debug", tbl)
    end
end

local function ignoreNextUpdate()
    local nextTick = vim.api.nvim_buf_get_changedtick(0) + 1
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

local function applyDelta(delta)
    local text_edits = {}
    for _, replacement in ipairs(delta) do
        local text_edit = {
            range = {
                start = replacement.range.anchor,
                ["end"] = replacement.range.head,
            },
            newText = replacement.replacement,
        }
        table.insert(text_edits, text_edit)
    end

    ignore_edits = true
    local changedtick_before = vim.api.nvim_buf_get_changedtick(0)
    vim.lsp.util.apply_text_edits(text_edits, 0, "utf-32")
    local changedtick_after = vim.api.nvim_buf_get_changedtick(0)
    ignore_edits = false

    debug({ changedtick_before = changedtick_before, changedtick_after = changedtick_after })

    daemonRevision = daemonRevision + 1
end

-- Take an operation from the daemon and apply it to the editor.
local function processOperationForEditor(method, parameters)
    if method == "edit" then
        local _uri = parameters.uri --[[@diagnostic disable-line]]
        local delta = parameters.delta.delta
        local theEditorRevision = parameters.delta.revision

        if theEditorRevision == editorRevision then
            applyDelta(delta)
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

-- Reset the state on editor side and re-open the current buffer
--
-- (this is to be called on buffer change, once we have the ability to detect that)
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

    local filename = "file://" .. vim.fs.basename(vim.api.nvim_buf_get_name(0))
    client.notify("open", { uri = filename })
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

-- Initialization function.
function Ethersync()
    if vim.fn.isdirectory(vim.fn.expand("%:p:h") .. "/.ethersync") ~= 1 then
        return
    end

    print("Ethersync activated!")

    connect()

    createCursor()

    prev_lines = vim.api.nvim_buf_get_lines(0, 0, -1, true)

    -- If there are no lines, set 'eol' to true. We didn't find a way to tell if the file contains '\n' or ''.
    if #prev_lines == 0 then
        vim.bo.eol = false
    end

    vim.api.nvim_buf_attach(0, false, {
        on_lines = function(
            _the_literal_string_lines --[[@diagnostic disable-line]],
            _buffer_handle --[[@diagnostic disable-line]],
            changedtick,
            first_line,
            last_line,
            new_last_line
        )
            -- line counts that we get called with are zero-based.
            -- last_line and new_last_line are exclusive

            debug({ first_line = first_line, last_line = last_line, new_last_line = new_last_line })
            local curr_lines = vim.api.nvim_buf_get_lines(0, 0, -1, true)

            -- Did the change come from us? If so, ignore it.
            if ignored_ticks[changedtick] then
                ignored_ticks[changedtick] = nil
                prev_lines = curr_lines
                return
            end

            -- Are we currently ignoring edits?
            if ignore_edits then
                return
            end

            editorRevision = editorRevision + 1

            debug({ curr_lines = curr_lines, prev_lines = prev_lines })
            local diff = sync.compute_diff(prev_lines, curr_lines, first_line, last_line, new_last_line, "utf-32", "\n")
            -- line/character indices in diff are zero-based.
            debug({ diff = diff })

            -- Sometimes, Vim deletes full lines by deleting the last line, plus an imaginary newline at the end. For example, to delete the second line, Vim would delete from (line: 1, column: 0) to (line: 2, column 0).
            -- But, in the case of deleting the last line, what we expect in the rest of Ethersync is to delete the newline *before* the line.
            -- So let's change the deleted range to (line: 0, column: [last character of the first line]) to (line: 1, column: [last character of the second line]).

            if
                diff.range["end"].line == #prev_lines
                and diff.range.start.line == #prev_lines - 1
                and diff.range["end"].character == 0
                and diff.range.start.character == 0
            then
                if diff.range.start.line > 0 then
                    diff.range.start.character = #prev_lines[diff.range.start.line]
                    diff.range.start.line = diff.range.start.line - 1
                    diff.range["end"].character = #prev_lines[diff.range["end"].line]
                    diff.range["end"].line = diff.range["end"].line - 1
                else
                    -- Special case: if start line already is 0, we can't shift the deletion backwards like that.
                    -- TODO: Find out whether or not there is a newline in the end?
                    diff.range["end"].character = #prev_lines[diff.range["end"].line]
                    diff.range["end"].line = diff.range["end"].line - 1
                end
            end

            debug({ fixed_diff = diff })

            local rev_delta = {
                delta = {
                    {
                        range = {
                            anchor = diff.range.start,
                            head = diff.range["end"],
                        },
                        replacement = diff.text,
                    },
                },
                revision = daemonRevision,
            }

            local uri = "file://" .. vim.api.nvim_buf_get_name(0)
            local params = { uri = uri, delta = rev_delta }

            if online then
                client.notify("edit", params)
            else
                table.insert(opQueueForDaemon, { "edit", params })
            end

            prev_lines = curr_lines
        end,
    })

    -- TODO: Re-enable this?
    --if vim.api.nvim_get_option_value("fixeol", { buf = 0 }) then
    --    if not vim.api.nvim_get_option_value("eol", { buf = 0 }) then
    --        utils.appendNewline()
    --        vim.api.nvim_set_option_value("eol", true, { buf = 0 })
    --    end
    --    vim.api.nvim_set_option_value("fixeol", false, { buf = 0 })
    --end

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

    local filename = "file://" .. vim.fs.basename(vim.api.nvim_buf_get_name(0))
    client.notify("close", { uri = filename })
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
