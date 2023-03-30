local ignored_ticks = {}
local sep = "\t"

local ethersyncClock = 0

local ns_id = vim.api.nvim_create_namespace('Ethersync')
local virtual_cursor
local server = vim.loop.new_tcp()

function indexToRowCol(index)
    local row = vim.fn.byte2line(index+1) - 1
    local col = index - vim.api.nvim_buf_get_offset(0, row)
    return row, col
end

function ignoreNextUpdate()
    local nextTick = vim.api.nvim_buf_get_changedtick(0)
    ignored_ticks[nextTick] = true
end

function rowColToIndex(row, col)
    return vim.fn.line2byte(row+1) + col-1
end

function insert(index, content)
    local row, col = indexToRowCol(index)
    ignoreNextUpdate()
    vim.api.nvim_buf_set_text(0, row, col, row, col, vim.split(content, "\n"))
end

function delete(index, length)
    local row, col = indexToRowCol(index)
    local rowEnd, colEnd = indexToRowCol(index + length)
    ignoreNextUpdate()
    vim.api.nvim_buf_set_text(0, row, col, rowEnd, colEnd, {""})
end

function setCursor(index, length)
    vim.schedule(function()
        if length == 0 or length == nil then
            length = 1
        end
        local row, col = indexToRowCol(index)
        local rowEnd, colEnd = indexToRowCol(index + length)

        vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
            id = virtual_cursor,
            hl_mode = 'combine',
            hl_group = 'TermCursor',
            end_col = colEnd,
            end_row = rowEnd
        })
    end)
end

function Ethersync()
    print('Ethersync activated!')
    --vim.opt.modifiable = false

    local row = 0
    local col = 0
    virtual_cursor = vim.api.nvim_buf_set_extmark(0, ns_id, row, col, {
        hl_mode = 'combine',
        hl_group = 'TermCursor',
        end_col = col+0
    })

    --setCursor(12,10)

    connect()

    local buf = vim.api.nvim_get_current_buf()
    local row, col = unpack(vim.api.nvim_win_get_cursor(0))
    vim.api.nvim_buf_attach(0, false, {
        on_bytes = function(the_string_bytes, buffer_handle, changedtick, start_row, start_column, byte_offset, old_end_row, old_end_column, old_end_byte_length, new_end_row, new_end_column, new_end_byte_length)
            -- Did the change come from us? If so, ignore it.
            if ignored_ticks[changedtick] then
                ignored_ticks[changedtick] = nil
                return
            end

            local new_content_lines = vim.api.nvim_buf_get_text(buffer_handle, start_row, start_column, start_row+new_end_row, start_column+new_end_column, {})
            local changed_string = table.concat(new_content_lines, "\n")

            local filename = vim.api.nvim_buf_get_name(0)

            if new_end_byte_length >= old_end_byte_length then
                server:write(vim.fn.join({"insert", filename, byte_offset, changed_string, vim.api.nvim_buf_get_changedtick(0), ethersyncClock}, sep))
            else
                server:write(vim.fn.join({"delete", filename, byte_offset, old_end_byte_length - new_end_byte_length, vim.api.nvim_buf_get_changedtick(0), ethersyncClock}, sep))
            end

            -- For testing, insert text at the virtual cursor.
            --vim.schedule(function()

            --    local row, col = unpack(vim.api.nvim_buf_get_extmark_by_id(0, ns_id, virtual_cursor, {}))
            --    local index = rowColToIndex(row, col)

            --    if new_end_byte_length >= old_end_byte_length then
            --        insert(index, changed_string)
            --    else
            --        local length = old_end_byte_length - new_end_byte_length
            --        delete(index-length, length)
            --    end
            --end)
        end
    })
end

function connect()
    server:connect("127.0.0.1", 9000, function (err)
        print(err)
    end)
    server:read_start(function(err, data)
        if err then
            print(err)
            return
        end
        if data then
            print(data)
            local parts = vim.split(data, sep)
            if parts[1] == "insert" then
                local index = tonumber(parts[2])
                local content = parts[3]
                local vimClock = tonumber(parts[4])
                local ethersyncClock = tonumber(parts[5])
                ethersyncClock = ethersyncClock + 1 -- next expected
                vim.schedule(function()
                    insert(index, content)
                end)
            elseif parts[1] == "delete" then
                local index = tonumber(parts[2])
                local length = tonumber(parts[3])
                local vimClock = tonumber(parts[4])
                local ethersyncClock = tonumber(parts[5])
                ethersyncClock = ethersyncClock + 1
                vim.schedule(function()
                    delete(index, length)
                end)
            elseif parts[1] == "cursor" then
                setCursor(tonumber(parts[2]), 1)
            end
        end
    end)
end

-- When new buffer is loaded, run Ethersync.
vim.api.nvim_exec([[
augroup Ethersync
    autocmd!
    autocmd BufEnter *.ethersync lua Ethersync()
augroup END
]], false)

vim.api.nvim_create_user_command('Ethersync', Ethersync, {})
vim.keymap.set('n', '<Leader>p', Ethersync)
