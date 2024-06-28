local M = {}
local user_cursors = {}
local cursor_namespace = vim.api.nvim_create_namespace("Ethersync")
local offset_encoding = "utf-32"
local cursor_timeout_ms = 30 * 1000

function is_forward(start_row, end_row, start_col, end_col)
    return (start_row < end_row) or (start_row == end_row and start_col <= end_col)
end

-- A new set of ranges means, we delete all existing ones for that user.
function M.setCursor(bufnr, user_id, ranges)
    if user_cursors[user_id] ~= nil then
        for _, cursor_buffer_tuple in ipairs(user_cursors[user_id]) do
            local old_cursor_id = cursor_buffer_tuple.cursor_id
            local old_bufnr = cursor_buffer_tuple.bufnr
            vim.api.nvim_buf_del_extmark(old_bufnr, cursor_namespace, old_cursor_id)
        end
    end
    user_cursors[user_id] = {}

    if not vim.api.nvim_buf_is_loaded(bufnr) then
        return
    end

    for _, range in ipairs(ranges) do
        -- Convert from LSP style ranges to Neovim style ranges.
        local start_row = range.start.line
        local start_col = vim.lsp.util._get_line_byte_from_position(bufnr, range.start, offset_encoding)
        local end_row = range["end"].line
        local end_col = vim.lsp.util._get_line_byte_from_position(bufnr, range["end"], offset_encoding)

        -- If the range is backwards, swap the start and end positions.
        if not is_forward(start_row, end_row, start_col, end_col) then
            start_row, end_row = end_row, start_row
            start_col, end_col = end_col, start_col
        end

        -- If the range is empty, expand the highlighted range by 1 to make it visible.
        if start_row == end_row and start_col == end_col then
            end_col = end_col + 1
        end

        local e = {
            start_row = start_row,
            start_col = start_col,
            end_row = end_row,
            end_col = end_col,
        }

        -- TODO:
        -- How can we display something at the end of lines?
        -- Virtual text, like the Copilot plugin?

        -- Try setting the extmark, ignore errors (which can happen at end of lines/buffers).
        pcall(function()
            local cursor_id = vim.api.nvim_buf_set_extmark(bufnr, cursor_namespace, e.start_row, e.start_col, {
                hl_mode = "combine",
                hl_group = "TermCursor",
                end_col = e.end_col,
                end_row = e.end_row,
            })
            vim.defer_fn(function()
                vim.api.nvim_buf_del_extmark(bufnr, cursor_namespace, cursor_id)
            end, cursor_timeout_ms)
            table.insert(user_cursors[user_id], { cursor_id = cursor_id, bufnr = bufnr })
        end)
    end
end

function M.trackCursor(bufnr, callback)
    vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI", "ModeChanged" }, {
        buffer = bufnr,
        callback = function()
            local ranges = {}

            local visualSelection = vim.fn.mode() == "v" or vim.fn.mode() == "V" or vim.fn.mode() == ""
            if visualSelection then
                local start_row, start_col = unpack(vim.api.nvim_win_get_cursor(0))
                local _, end_row, end_col = unpack(vim.fn.getpos("v"))

                -- When using getpos(), the column is 1-indexed, but we want it to be 0-indexed.
                end_col = end_col - 1

                -- We're not sure why this is necessary.
                end_col = end_col - 1

                -- Include the last character of the visual selection.
                if is_forward(start_row, end_row, start_col, end_col) then
                    end_col = end_col + 1
                else
                    start_col = start_col + 1
                end

                -- If we're in linewise visual mode, expand the range to include the entire line(s).
                if vim.fn.mode() == "V" then
                    if is_forward(start_row, end_row, start_col, end_col) then
                        start_col = 0
                        end_col = vim.fn.strlen(vim.fn.getline(end_row)) - 1
                    else
                        start_col = vim.fn.strlen(vim.fn.getline(start_row))
                        end_col = -1
                    end
                end

                if vim.fn.mode() == "v" or vim.fn.mode() == "V" then
                    local range = vim.lsp.util.make_given_range_params(
                        { start_row, start_col },
                        { end_row, end_col },
                        bufnr,
                        offset_encoding
                    ).range
                    ranges = { range }
                elseif vim.fn.mode() == "" then
                    -- Blockwise visual mode! Calculate the individual pieces.
                    -- TODO: There's still a off-by-one error here depending on the left-right direction of the visual blockwise selection.
                    -- TODO: Re-using the same colums doesn't seem right, there's wrong behaviour for umlauts/Unicode characters.
                    local smaller_row = math.min(start_row, end_row)
                    local larger_row = math.max(start_row, end_row)
                    for row = smaller_row, larger_row do
                        local range = vim.lsp.util.make_given_range_params(
                            { row, start_col },
                            { row, end_col },
                            bufnr,
                            offset_encoding
                        ).range
                        table.insert(ranges, range)
                    end
                end
            else
                local range = vim.lsp.util.make_range_params(0, offset_encoding).range
                ranges = { range }
            end

            -- Rename from start/end schema to anchor/head.
            for _, range in ipairs(ranges) do
                range.anchor = range.start
                range.head = range["end"]
                range.start = nil
                range["end"] = nil
            end

            callback(ranges)
        end,
    })
end

return M
