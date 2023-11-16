local connection = require("connection")
local utils = require("utils")

-- Used to store the changedtick of the buffer when we make changes to it.
-- We do this to avoid infinite loops, where we make a change, which would
-- trigger normally an "on_bytes" event.
local ignored_ticks = {}

local ns_id = vim.api.nvim_create_namespace("Ethersync")
local virtual_cursor
local conn = connection.new_connection()

local function ignoreNextUpdate()
    local nextTick = vim.api.nvim_buf_get_changedtick(0)
    ignored_ticks[nextTick] = true
end

-- Insert a string into the current buffer at a specified UTF-16 code unit index.
local function insert(index, content)
    local charIndex = utils.UTF16CodeUnitOffsetToCharOffset(index)
    local row, col = utils.indexToRowCol(charIndex)
    ignoreNextUpdate()
    vim.api.nvim_buf_set_text(0, row, col, row, col, vim.split(content, "\n"))
end

-- Delete a string from the current buffer at a specified UTF-16 code unit index.
local function delete(index, length)
    local charIndex = utils.UTF16CodeUnitOffsetToCharOffset(index)
    local row, col = utils.indexToRowCol(charIndex)
    local charIndexEnd = utils.UTF16CodeUnitOffsetToCharOffset(index + length)
    local rowEnd, colEnd = utils.indexToRowCol(charIndexEnd)
    ignoreNextUpdate()
    vim.api.nvim_buf_set_text(0, row, col, rowEnd, colEnd, { "" })
end

local function createCursor()
    local row = 0
    local col = 0
    virtual_cursor = vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
        hl_mode = "combine",
        hl_group = "TermCursor",
        end_col = col + 0,
    })
end

-- Set the cursor position in the current buffer. If head and anchor are different,
-- a visual selection is created. head and anchor are in UTF-16 code units.
local function setCursor(head, anchor)
    vim.schedule(function()
        if head == anchor then
            anchor = head + 1
        end

        if head > anchor then
            head, anchor = anchor, head
        end

        -- If the cursor is at the end of the buffer, don't show it.
        -- TODO: Calculate in UTF-16 code units.
        if head == vim.fn.strchars(vim.fn.join(vim.api.nvim_buf_get_lines(0, 0, -1, true), "\n")) then
            return
        end

        local headChar = utils.UTF16CodeUnitOffsetToCharOffset(head)
        local row, col = utils.indexToRowCol(headChar)
        local anchorChar = utils.UTF16CodeUnitOffsetToCharOffset(anchor)
        local rowAnchor, colAnchor = utils.indexToRowCol(anchorChar)

        vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
            id = virtual_cursor,
            hl_mode = "combine",
            hl_group = "TermCursor",
            end_col = colAnchor,
            end_row = rowAnchor,
        })
    end)
end

-- Start a read loop, which reads messages from the Ethersync daemon.
local function start_read()
    conn:read(function(err, message)
        if err then
            print("Error: " .. err)
        else
            local pretty_printed = vim.fn.json_encode(message)
            print("Received message: " .. pretty_printed)
            if message[1] == "insert" then
                local filename = message[2]
                local index = tonumber(message[3])
                local content = message[4]
                vim.schedule(function()
                    if filename == vim.fs.basename(vim.api.nvim_buf_get_name(0)) then
                        insert(index, content)
                    end
                end)
            elseif message[1] == "delete" then
                local filename = message[2]
                local index = tonumber(message[3])
                local length = tonumber(message[4])
                vim.schedule(function()
                    if filename == vim.fs.basename(vim.api.nvim_buf_get_name(0)) then
                        delete(index, length)
                    end
                end)
            elseif message[1] == "cursor" then
                --local filename = message[2]
                local head = tonumber(message[3])
                local anchor = tonumber(message[4])
                -- TODO: Check filename.
                --if filename == vim.fs.basename(vim.api.nvim_buf_get_name(0)) then
                setCursor(head, anchor)
                --end
            end
        end
    end)
end

-- Initialization function.
function Ethersync()
    if vim.fn.isdirectory(vim.fn.expand("%:p:h") .. "/.ethersync") ~= 1 then
        return
    end

    print("Ethersync activated!")

    conn:connect("127.0.0.1", 9000, function(err)
        if err then
            print("Could not connect to Ethersync daemon: " .. err)
        else
            start_read()
        end
    end)

    createCursor()

    vim.api.nvim_buf_attach(0, false, {
        on_bytes = function(
            the_string_bytes,
            buffer_handle,
            changedtick,
            start_row,
            start_column,
            byte_offset,
            old_end_row,
            old_end_column,
            old_end_byte_length,
            new_end_row,
            new_end_column,
            new_end_byte_length
        )
            -- Did the change come from us? If so, ignore it.
            if ignored_ticks[changedtick] then
                ignored_ticks[changedtick] = nil
                return
            end

            conn:write({
                byte_offset = byte_offset,
                new_end_byte_length = new_end_byte_length,
                old_end_byte_length = old_end_byte_length,
            })

            local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
            local content = utils.contentOfCurrentBuffer()

            -- TODO: Calculate in UTF-16 code units.
            if byte_offset + new_end_byte_length > vim.fn.strlen(content) then
                -- Tried to insert something *after* the end of the (resulting) file.
                -- I think this is probably a bug, that happens when you use the 'o' command, for example.
                byte_offset = vim.fn.strlen(content) - new_end_byte_length
            end

            local charOffset = utils.byteOffsetToCharOffset(byte_offset)
            local oldEndChar = utils.byteOffsetToCharOffset(byte_offset + old_end_byte_length)
            local newEndChar = utils.byteOffsetToCharOffset(byte_offset + new_end_byte_length)

            local newEndCharLength = newEndChar - charOffset

            local charOffsetUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(charOffset)
            local oldEndCharUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(oldEndChar)
            local newEndCharUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(newEndChar)

            local oldEndCharUTF16CodeUnitsLength = oldEndCharUTF16CodeUnits - charOffsetUTF16CodeUnits
            local newEndCharUTF16CodeUnitsLength = newEndCharUTF16CodeUnits - charOffsetUTF16CodeUnits

            conn:write({ content = content })
            conn:write({
                charOffset = charOffsetUTF16CodeUnits,
                oldEndChar = oldEndCharUTF16CodeUnits,
                newEndChar = newEndCharUTF16CodeUnits,
                oldEndCharLength = oldEndCharUTF16CodeUnitsLength,
                newEndCharLength = newEndCharUTF16CodeUnitsLength,
            })

            -- TODO: For snippet expansion, for example, a deletion (of the snippet text) takes place, which is not accounted for here.

            if oldEndCharUTF16CodeUnitsLength > 0 then
                conn:write({ "delete", filename, charOffsetUTF16CodeUnits, oldEndCharUTF16CodeUnitsLength })
            end

            if newEndCharUTF16CodeUnitsLength > 0 then
                local insertedString = vim.fn.strcharpart(content, charOffset, newEndCharLength)
                conn:write({ "insert", filename, charOffsetUTF16CodeUnits, insertedString })
            end
        end,
    })

    vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI", "ModeChanged" }, {
        callback = function()
            local row, col = unpack(vim.api.nvim_win_get_cursor(0))
            local head = utils.rowColToIndex(row - 1, col)
            local headUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(head)

            if headUTF16CodeUnits == -1 then
                -- TODO what happens here?
                return
            end

            -- Is there a visual selection?
            local visualSelection = vim.fn.mode() == "v" or vim.fn.mode() == "V" or vim.fn.mode() == ""

            local anchorUTF16CodeUnits = headUTF16CodeUnits
            if visualSelection then
                local _, rowV, colV = unpack(vim.fn.getpos("v"))
                local anchor = utils.rowColToIndex(rowV - 1, colV)
                anchorUTF16CodeUnits = utils.charOffsetToUTF16CodeUnitOffset(anchor)
                if headUTF16CodeUnits > anchorUTF16CodeUnits then
                else
                    headUTF16CodeUnits = headUTF16CodeUnits + 1
                    anchorUTF16CodeUnits = anchorUTF16CodeUnits + 1
                end
            end
            local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
            conn:write({ "cursor", filename, headUTF16CodeUnits, anchorUTF16CodeUnits })
        end,
    })
end

-- When new buffer is loaded, run Ethersync.
vim.api.nvim_exec(
    [[
augroup Ethersync
    autocmd!
    autocmd BufEnter * lua Ethersync()
augroup END
]],
    false
)

-- Here are two other ways to run Ethersync:
vim.api.nvim_create_user_command("Ethersync", Ethersync, {})
vim.keymap.set("n", "<Leader>p", Ethersync)
