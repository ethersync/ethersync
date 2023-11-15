local connection = require("connection")
local utils = require("utils")

local ignored_ticks = {}

local ns_id = vim.api.nvim_create_namespace('Ethersync')
local virtual_cursor
local conn = connection.new_connection()

local function ignoreNextUpdate()
    local nextTick = vim.api.nvim_buf_get_changedtick(0)
    ignored_ticks[nextTick] = true
end

local function insert(index, content)
    local row, col = utils.indexToRowCol(index)
    ignoreNextUpdate()
    vim.api.nvim_buf_set_text(0, row, col, row, col, vim.split(content, "\n"))
end

local function delete(index, length)
    local row, col = utils.indexToRowCol(index)
    local rowEnd, colEnd = utils.indexToRowCol(index + length)
    ignoreNextUpdate()
    vim.api.nvim_buf_set_text(0, row, col, rowEnd, colEnd, { "" })
end

local function setCursor(head, anchor)
    vim.schedule(function()
        if head == anchor then
            anchor = head + 1
        end

        if head > anchor then
            head, anchor = anchor, head
        end

        -- If the cursor is at the end of the buffer, don't show it.
        if head == vim.fn.strchars(vim.fn.join(vim.api.nvim_buf_get_lines(0, 0, -1, true), "\n")) then
            return
        end

        local row, col = utils.indexToRowCol(head)
        local rowAnchor, colAnchor = utils.indexToRowCol(anchor)

        vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
            id = virtual_cursor,
            hl_mode = 'combine',
            hl_group = 'TermCursor',
            end_col = colAnchor,
            end_row = rowAnchor
        })
    end)
end

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
                local filename = message[2]
                local head = tonumber(message[3])
                local anchor = tonumber(message[4])
                if filename == vim.fs.basename(vim.api.nvim_buf_get_name(0)) then
                    setCursor(head, anchor)
                end
            end
        end
    end)
end

function Ethersync()
    if vim.fn.isdirectory(vim.fn.expand('%:p:h') .. '/.ethersync') ~= 1 then
        return
    end

    print('Ethersync activated!')
    --vim.opt.modifiable = false

    conn:connect("127.0.0.1", 9000, function(err)
        if err then
            print("Could not connect to Ethersync daemon: " .. err)
        else
            start_read()
        end
    end)

    --local row = 0
    --local col = 0
    --virtual_cursor = vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
    --    hl_mode = 'combine',
    --    hl_group = 'TermCursor',
    --    end_col = col + 0
    --})

    --setCursor(12,10)

    --connect()

    vim.api.nvim_buf_attach(0, false, {
        on_bytes = function(the_string_bytes, buffer_handle, changedtick, start_row, start_column, byte_offset,
                            old_end_row, old_end_column, old_end_byte_length, new_end_row, new_end_column,
                            new_end_byte_length)
            -- Did the change come from us? If so, ignore it.
            if ignored_ticks[changedtick] then
                ignored_ticks[changedtick] = nil
                return
            end

            --print("start_row: " .. start_row)
            --print("num lines: " .. vim.fn.line('$'))
            --local num_rows = vim.fn.line('$')
            --if start_row == num_rows-1 and start_column == 0 and new_end_column == 0 then
            --    -- Edit is after the end of the buffer. Ignore it.
            --    return
            --end

            --local new_content_lines = vim.api.nvim_buf_get_text(buffer_handle, start_row, start_column, start_row + new_end_row, start_column + new_end_column, {})

            conn:write({ byte_offset = byte_offset, new_end_byte_length = new_end_byte_length,
                old_end_byte_length = old_end_byte_length })

            local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
            local content = utils.contentOfCurrentBuffer()

            if byte_offset + new_end_byte_length > vim.fn.strlen(content) then
                -- Tried to insert something *after* the end of the (resulting) file.
                -- I think this is probably a bug, that happens when you use the 'o' command, for example.
                byte_offset = vim.fn.strlen(content) - new_end_byte_length
            end

            local charOffset = utils.byteOffsetToCharOffset(byte_offset)
            local oldEndChar = utils.byteOffsetToCharOffset(byte_offset + old_end_byte_length)
            local newEndChar = utils.byteOffsetToCharOffset(byte_offset + new_end_byte_length)
            local oldEndCharLength = oldEndChar - charOffset
            local newEndCharLength = newEndChar - charOffset

            conn:write({ content = content })
            conn:write({ charOffset = charOffset, oldEndChar = oldEndChar, newEndChar = newEndChar,
                oldEndCharLength = oldEndCharLength, newEndCharLength = newEndCharLength })

            -- TODO: For snippet expansion, for example, a deletion (of the snippet text) takes place, which is not accounted for here.

            if oldEndCharLength > 0 then
                conn:write({ "delete", filename, charOffset, oldEndCharLength })
            end

            if newEndCharLength > 0 then
                local insertedString = vim.fn.strcharpart(content, charOffset, newEndCharLength)
                conn:write({ "insert", filename, charOffset, insertedString })
            end
        end
    })

    vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI" }, {
        callback = function()
            local row, col = unpack(vim.api.nvim_win_get_cursor(0))
            local head = utils.rowColToIndex(row - 1, col)

            if head == -1 then
                -- TODO what happens here?
                return
            end

            -- Is there a visual selection?
            local visualSelection = vim.fn.mode() == 'v' or vim.fn.mode() == 'V' or vim.fn.mode() == ''

            local anchor = head
            if visualSelection then
                local _, rowV, colV = unpack(vim.fn.getpos("v"))
                anchor = utils.rowColToIndex(rowV - 1, colV)
                if head < anchor then
                else
                    head = head + 1
                    anchor = anchor - 1
                end
            end
            local filename = vim.fs.basename(vim.api.nvim_buf_get_name(0))
            --conn:write({ "cursor", filename, head, anchor })
        end })
end

-- When new buffer is loaded, run Ethersync.
vim.api.nvim_exec([[
augroup Ethersync
    autocmd!
    autocmd BufEnter * lua Ethersync()
augroup END
]], false)

-- Here are two other ways to run Ethersync:
vim.api.nvim_create_user_command('Ethersync', Ethersync, {})
vim.keymap.set('n', '<Leader>p', Ethersync)
