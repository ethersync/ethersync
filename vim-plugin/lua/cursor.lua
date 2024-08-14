-- SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local M = {}

-- This variable is a table that maps user IDs to a list of cursors.
-- Each cursor has an URI, an LSP range, an optional name,
-- and, if we already created it, an optional extmark information,
-- (including the buffer number and the extmark id).
-- { user_id => {
--      name: string,
--      cursors: { uri, range, extmark: { bufnr: int, id: int}?}}
--    }
-- }
local user_cursors = {}
local cursor_namespace = vim.api.nvim_create_namespace("Ethersync")
local offset_encoding = "utf-32"
local cursor_timeout_ms = 300 * 1000

local function show_cursor_information(name, cursor)
    return name .. " @ " .. vim.uri_to_fname(cursor.uri) .. ":" .. cursor.range.start.line + 1
end

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
function M.set_cursor(uri, user_id, name, ranges)
    -- Find correct buffer to apply edits to.
    local bufnr = vim.uri_to_bufnr(uri)

    if user_cursors[user_id] then
        for _, user_cursor in ipairs(user_cursors[user_id].cursors) do
            if user_cursor.extmark then
                local old_id = user_cursor.extmark.id
                local old_bufnr = user_cursor.extmark.bufnr
                vim.api.nvim_buf_del_extmark(old_bufnr, cursor_namespace, old_id)
            end
        end
    end
    user_cursors[user_id] = { name = name, cursors = {} }

    if not vim.api.nvim_buf_is_loaded(bufnr) then
        -- TODO: Should we also implement a timeout here?
        for _, range in ipairs(ranges) do
            table.insert(user_cursors[user_id].cursors, { uri = uri, range = range, extmark = nil })
        end
        return
    end

    for i, range in ipairs(ranges) do
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
        if i == 1 and name then
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
                { uri = uri, range = range, extmark = { id = extmark_id, bufnr = bufnr } }
            )
        end)
    end
end

function M.track_cursor(bufnr, callback)
    vim.api.nvim_create_autocmd({ "CursorMoved", "CursorMovedI", "ModeChanged" }, {
        buffer = bufnr,
        callback = function()
            local ranges = {}

            -- TODO: Split this code into multiple functions.
            local visualSelection = vim.fn.mode() == "v" or vim.fn.mode() == "V" or vim.fn.mode() == ""
            if visualSelection then
                -- This is the "active end" that the protocol talks about.
                local end_row, end_col = unpack(vim.api.nvim_win_get_cursor(0))
                -- Whereas this corresponds the the "anchor" in other range data structures.
                local _, start_row, start_col = unpack(vim.fn.getpos("v"))

                -- When using getpos(), the column is 1-indexed, but we want it to be 0-indexed.
                start_col = start_col - 1

                if vim.fn.mode() == "v" then
                    -- TODO: This is not in "screen columns", but in "bytes"!
                    if not is_forward(start_row, end_row, start_col, end_col) then
                        -- If the selection is backwards, we need to extend it on both ends for some reason.
                        end_col = end_col - 1
                        start_col = start_col + 1
                    end
                elseif vim.fn.mode() == "V" then
                    -- If we're in linewise visual mode, expand the range to include the entire line(s).
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

            callback(ranges)
        end,
    })
end

function M.jump_to_cursor()
    local descriptions = {}
    local locations = {}
    local max_width = 10
    for _, data in pairs(user_cursors) do
        local _, cursor = next(data.cursors)
        if cursor then
            local description = show_cursor_information(data.name, cursor)
            local desc_width = vim.fn.strdisplaywidth(description)
            if desc_width > max_width then
                max_width = desc_width
            end

            table.insert(descriptions, description)

            table.insert(locations, {
                targetUri = cursor.uri,
                targetRange = cursor.range,
                targetSelectionRange = cursor.range,
            })
        end
    end

    if #locations == 0 then
        print("No cursors to jump to.")
        return
    elseif #locations == 1 then
        -- Jump immediately.
        vim.lsp.util.jump_to_location(locations[1], offset_encoding, true)
        return
    end

    local buf = vim.api.nvim_create_buf(false, true)
    vim.api.nvim_buf_set_lines(buf, 0, -1, true, descriptions)
    vim.bo[buf].modifiable = false

    local opts = {
        relative = "cursor",
        width = max_width,
        height = #descriptions,
        col = 0,
        row = 1,
        anchor = "NW",
        style = "minimal",
        border = "single",
    }
    local win = vim.api.nvim_open_win(buf, true, opts)

    vim.api.nvim_buf_set_keymap(buf, "n", "<CR>", "", {
        callback = function()
            local line_number = vim.fn.line(".")
            local location = locations[line_number]
            vim.api.nvim_win_close(win, true)
            vim.lsp.util.jump_to_location(location, offset_encoding, true)
        end,
    })
    vim.api.nvim_buf_set_keymap(buf, "n", "<Esc>", "", {
        callback = function()
            vim.api.nvim_win_close(win, true)
        end,
    })
    vim.api.nvim_create_autocmd("BufLeave", {
        buffer = buf,
        callback = function()
            vim.api.nvim_win_close(win, true)
        end,
    })
end

function M.list_cursors()
    local message = ""

    if next(user_cursors) == nil then
        message = "No cursors."
    else
        for _, data in pairs(user_cursors) do
            local name = data.name
            local cursors = data.cursors
            message = message .. name .. ":"
            if #cursors == 0 then
                message = message .. " No cursors"
            elseif #cursors == 1 then
                message = show_cursor_information(name, cursors[1])
            else
                message = message .. " Multiple cursors in " .. vim.uri_to_fname(cursors[1].uri)
            end
        end
    end

    return message
end

return M
