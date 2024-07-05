local debug = require("logging").debug

local M = {}

-- This variable is a table that maps user IDs to a list of cursors.
-- Each cursor has an URI, an LSP range, an optional name,
-- and, if we already created it, an optional extmark information,
-- (including the buffer number and the extmark id).
-- { user_id => {
--      name: string,
--      cursors: { uri, range, name?, extmark: { bufnr: int, id: int}?}}
--    }
-- }
local user_cursors = {}
local cursor_namespace = vim.api.nvim_create_namespace("Ethersync")
local offset_encoding = "utf-32"
local cursor_timeout_ms = 30 * 1000

-- https://www.reddit.com/r/neovim/comments/152bs5t/unable_to_render_comments_in_the_color_id_like/
vim.api.nvim_create_autocmd("ColorScheme", {
    pattern = "*",
    callback = function()
        vim.api.nvim_set_hl(0, "EthersyncUsername", { fg = "#808080", ctermfg = 12 })
    end,
})
vim.api.nvim_exec_autocmds("ColorScheme", {})

local function is_forward(start_row, end_row, start_col, end_col)
    return (start_row < end_row) or (start_row == end_row and start_col <= end_col)
end

-- A new set of ranges means, we delete all existing ones for that user.
function M.setCursor(uri, user_id, name, ranges)
    -- Find correct buffer to apply edits to.
    local bufnr = vim.uri_to_bufnr(uri)

    -- Convert range format to LSP's "start"/"end" format.
    local ranges_se = {}
    for _, range in ipairs(ranges) do
        table.insert(ranges_se, {
            start = range.anchor,
            ["end"] = range.head,
        })
    end

    if user_cursors[user_id] ~= nil then
        for _, user_cursor in ipairs(user_cursors[user_id]) do
            if user_cursor.extmark ~= nil then
                local old_id = user_cursor.extmark.id
                local old_bufnr = user_cursor.extmark.bufnr
                vim.api.nvim_buf_del_extmark(old_bufnr, cursor_namespace, old_id)
            end
        end
    end
    user_cursors[user_id] = { name = name, cursors = {} }

    if not vim.api.nvim_buf_is_loaded(bufnr) then
        -- TODO: Should we also implement a timeout here?
        for _, range in ipairs(ranges_se) do
            table.insert(user_cursors[user_id].cursors, { uri = uri, range = range, name = name, extmark = nil })
        end
        return
    end

    for i, range in ipairs(ranges_se) do
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
            local bytes_in_end_row = vim.fn.strlen(vim.fn.getline(end_row + 1))
            if bytes_in_end_row > end_col then
                -- Note: Instead of 1, we should actually add the byte length of the character at this position.
                end_col = end_col + 1
            elseif bytes_in_end_row > 0 then
                -- This highlights the last character in the row.
                start_col = start_col - 1
            end
        end

        local e = {
            start_row = start_row,
            start_col = start_col,
            end_row = end_row,
            end_col = end_col,
        }

        local virt_text = {}
        if i == 1 and name ~= nil then
            virt_text = { { name, "EthersyncUsername" } }
        end

        -- Try setting the extmark, ignore errors (which can happen at end of lines/buffers).
        pcall(function()
            local extmark_id = vim.api.nvim_buf_set_extmark(bufnr, cursor_namespace, e.start_row, e.start_col, {
                hl_mode = "combine",
                hl_group = "TermCursor",
                end_col = e.end_col,
                end_row = e.end_row,
                virt_text = virt_text,
            })
            vim.defer_fn(function()
                vim.api.nvim_buf_del_extmark(bufnr, cursor_namespace, extmark_id)
                for _, user_cursor in ipairs(user_cursors[user_id].cursors) do
                    if user_cursor.extmark ~= nil and user_cursor.extmark.id == extmark_id then
                        -- If we find our own extmark_id, we can remove all ranges,
                        -- because they were all created at the same time.
                        user_cursors[user_id].cursors = {}
                        break
                    end
                end
            end, cursor_timeout_ms)

            table.insert(
                user_cursors[user_id].cursors,
                { uri = uri, range = range, name = name, extmark = { id = extmark_id, bufnr = bufnr } }
            )
        end)
    end
end

function M.trackCursor(bufnr, callback)
    vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI", "ModeChanged" }, {
        buffer = bufnr,
        callback = function()
            local ranges = {}

            -- TODO: Split this code into multiple functions.
            local visualSelection = vim.fn.mode() == "v" or vim.fn.mode() == "V" or vim.fn.mode() == ""
            if visualSelection then
                local start_row, start_col = unpack(vim.api.nvim_win_get_cursor(0))
                local _, end_row, end_col = unpack(vim.fn.getpos("v"))

                -- When using getpos(), the column is 1-indexed, but we want it to be 0-indexed.
                end_col = end_col - 1

                if vim.fn.mode() == "v" or vim.fn.mode() == "V" then
                    -- We're not sure why this is necessary.
                    end_col = end_col - 1

                    -- Include the last character of the visual selection.
                    -- TODO: This is not in "screen columns", but in "bytes"!
                    if is_forward(start_row, end_row, start_col, end_col) then
                        end_col = end_col + 1
                    else
                        start_col = start_col + 1
                    end
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
                    -- We are in blockwise visual mode. Calculate the individual pieces.

                    -- This calculation is a bit more involved, because Vim forms a blockwise range visually, going by the
                    -- "display cells", so that the range is always rectangular. We need to perform our own calculations with
                    -- these display cells to make sure that we send out the same ranges.

                    -- TODO: There are still some inconsistencies; when the cursor is inside a multi-column character, other lines might be too short.

                    -- At this point, start_col and end_col are zero-indexed, in bytes, and related to the position in front of the cursor.

                    local start_line = vim.fn.getline(start_row)
                    local end_line = vim.fn.getline(end_row)

                    local string_to_start = string.sub(start_line, 0, start_col)
                    local string_to_end = string.sub(end_line, 0, end_col)

                    -- These are the widths of the strings in "display cells".
                    local cells_to_start = vim.fn.strdisplaywidth(string_to_start)
                    local cells_to_end = vim.fn.strdisplaywidth(string_to_end)

                    local smaller_cell = math.min(cells_to_start, cells_to_end)
                    local larger_cell = math.max(cells_to_start, cells_to_end)

                    local smaller_row = math.min(start_row, end_row)
                    local larger_row = math.max(start_row, end_row)

                    for row = smaller_row, larger_row do
                        local bytes_start = vim.fn.virtcol2col(bufnr, row, smaller_cell + 1) - 1
                        local bytes_end = vim.fn.virtcol2col(bufnr, row, larger_cell + 1) - 1

                        if bytes_start ~= -1 then
                            -- The line is not empty, add a range.
                            local range = vim.lsp.util.make_given_range_params(
                                { row, bytes_start },
                                { row, bytes_end },
                                bufnr,
                                offset_encoding
                            ).range
                            table.insert(ranges, range)
                        end
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

local function get_first_user_cursor()
    local _, first_user_cursors = next(user_cursors.cursors)

    if first_user_cursors == nil then
        return nil
    end

    local _, first_cursor = next(first_user_cursors)

    if first_cursor == nil then
        return nil
    end

    return first_cursor
end

function M.JumpToCursor()
    local cursor = get_first_user_cursor()
    if cursor == nil then
        return
    end

    local location = {
        targetUri = cursor.uri,
        targetRange = cursor.range,
        targetSelectionRange = cursor.range,
    }
    vim.lsp.util.jump_to_location(location, offset_encoding, true)
end

function M.ListCursors()
    local message = ""

    if next(user_cursors) == nil then
        message = "No cursors."
    else
        for user_id, data in pairs(user_cursors) do
            local name = data.name
            local cursors = data.cursors
            message = message .. name .. ":"
            if #cursors == 0 then
                message = message .. " No cursors"
            elseif #cursors == 1 then
                message = message
                    .. " "
                    .. cursors[1].uri
                    .. ":"
                    .. cursors[1].range.start.line
                    .. ":"
                    .. cursors[1].range.start.character
            else
                message = message .. " Multiple cursors in " .. cursors[1].uri
            end
        end
    end

    return message
end

return M
