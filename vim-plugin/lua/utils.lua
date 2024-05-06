local M = {}

function M.appendNewline()
    print("Appending newline...")
    vim.cmd("normal! Go")
end

-- The following functions are taken from the Neovim source code:
-- https://github.com/neovim/neovim/blob/master/runtime/lua/vim/lsp/util.lua

--- Gets the zero-indexed lines from the given buffer.
---
---@param bufnr integer bufnr to get the lines from
---@param rows integer[] zero-indexed line numbers
---@return table<integer, string>|string a table mapping rows to lines
local function get_lines(bufnr, rows)
    rows = type(rows) == "table" and rows or { rows }

    local lines = {}
    for _, row in ipairs(rows) do
        lines[row] = (vim.api.nvim_buf_get_lines(bufnr, row, row + 1, false) or { "" })[1]
    end
    return lines
end

--- Gets the zero-indexed line from the given buffer.
--- Works on unloaded buffers by reading the file using libuv to bypass buf reading events.
--- Falls back to loading the buffer and nvim_buf_get_lines for buffers with non-file URI.
---
---@param bufnr integer
---@param row integer zero-indexed line number
---@return string the line at row in filename
local function get_line(bufnr, row)
    return get_lines(bufnr, { row })[row]
end

--- Applies a list of text edits to a buffer.
---@param text_edits table list of `TextEdit` objects
---@param bufnr integer Buffer id
---@param offset_encoding string utf-8|utf-16|utf-32
---@see https://microsoft.github.io/language-server-protocol/specifications/specification-current/#textEdit
function M.apply_text_edits(text_edits, bufnr, offset_encoding)
    vim.validate({
        text_edits = { text_edits, "t", false },
        bufnr = { bufnr, "number", false },
        offset_encoding = { offset_encoding, "string", false },
    })
    if not next(text_edits) then
        return
    end

    if not vim.api.nvim_buf_is_loaded(bufnr) then
        vim.fn.bufload(bufnr)
    end
    vim.bo[bufnr].buflisted = true

    -- Fix reversed range and indexing each text_edits
    local index = 0
    text_edits = vim.tbl_map(function(text_edit)
        index = index + 1
        text_edit._index = index

        if
            text_edit.range.start.line > text_edit.range["end"].line
            or text_edit.range.start.line == text_edit.range["end"].line
                and text_edit.range.start.character > text_edit.range["end"].character
        then
            local start = text_edit.range.start
            text_edit.range.start = text_edit.range["end"]
            text_edit.range["end"] = start
        end
        return text_edit
    end, text_edits)

    -- Sort text_edits
    table.sort(text_edits, function(a, b)
        if a.range.start.line ~= b.range.start.line then
            return a.range.start.line > b.range.start.line
        end
        if a.range.start.character ~= b.range.start.character then
            return a.range.start.character > b.range.start.character
        end
        if a._index ~= b._index then
            return a._index > b._index
        end
    end)

    -- save and restore local marks since they get deleted by nvim_buf_set_lines
    local marks = {}
    for _, m in pairs(vim.fn.getmarklist(bufnr)) do
        if m.mark:match("^'[a-z]$") then
            marks[m.mark:sub(2, 2)] = { m.pos[2], m.pos[3] - 1 } -- api-indexed
        end
    end

    -- Apply text edits.
    --local has_eol_text_edit = false
    local disable_eol = false
    for _, text_edit in ipairs(text_edits) do
        -- Normalize line ending
        text_edit.newText, _ = string.gsub(text_edit.newText, "\r\n?", "\n")

        -- Convert from LSP style ranges to Neovim style ranges.
        local e = {
            start_row = text_edit.range.start.line,
            start_col = vim.lsp.util._get_line_byte_from_position(bufnr, text_edit.range.start, offset_encoding),
            end_row = text_edit.range["end"].line,
            end_col = vim.lsp.util._get_line_byte_from_position(bufnr, text_edit.range["end"], offset_encoding),
            text = vim.split(text_edit.newText, "\n", { plain = true }),
        }

        local max = vim.api.nvim_buf_line_count(bufnr)
        -- If the whole edit is after the lines in the buffer we can simply add the new text to the end
        -- of the buffer.
        if max <= e.start_row then
            vim.api.nvim_buf_set_lines(bufnr, max, max, false, e.text)
        else
            local last_line_len = #(get_line(bufnr, math.min(e.end_row, max - 1)) or "")
            -- Some LSP servers may return +1 range of the buffer content but nvim_buf_set_text can't
            -- accept it so we should fix it here.
            if max <= e.end_row then
                e.end_row = max - 1
                e.end_col = last_line_len
                --has_eol_text_edit = true
                disable_eol = true
                -- "a" + 'eol' + replace((0,1), (1,0), "") => "a" + 'noeol'
                -- "a" + 'eol' + replace((0,1), (1,0), "\n\n") => "a\n\n" + 'noeol' (I guess?)
            else
                -- If the replacement is over the end of a line (i.e. e.end_col is out of bounds and the
                -- replacement text ends with a newline We can likely assume that the replacement is assumed
                -- to be meant to replace the newline with another newline and we need to make sure this
                -- doesn't add an extra empty line. E.g. when the last line to be replaced contains a '\r'
                -- in the file some servers (clangd on windows) will include that character in the line
                -- while nvim_buf_set_text doesn't count it as part of the line.
                if
                    e.end_col > last_line_len
                    and #text_edit.newText > 0
                    and string.sub(text_edit.newText, -1) == "\n"
                then
                    table.remove(e.text, #e.text)
                end
            end
            -- Make sure we don't go out of bounds for e.end_col
            e.end_col = math.min(last_line_len, e.end_col)

            vim.api.nvim_buf_set_text(bufnr, e.start_row, e.start_col, e.end_row, e.end_col, e.text)
        end
    end

    local max = vim.api.nvim_buf_line_count(bufnr)

    -- no need to restore marks that still exist
    for _, m in pairs(vim.fn.getmarklist(bufnr)) do
        marks[m.mark:sub(2, 2)] = nil
    end
    -- restore marks
    for mark, pos in pairs(marks) do
        if pos then
            -- make sure we don't go out of bounds
            pos[1] = math.min(pos[1], max)
            pos[2] = math.min(pos[2], #(get_line(bufnr, pos[1] - 1) or ""))
            vim.api.nvim_buf_set_mark(bufnr or 0, mark, pos[1], pos[2], {})
        end
    end

    if disable_eol then
        vim.bo.eol = false
    end
    --local fix_eol = has_eol_text_edit
    --fix_eol = fix_eol and (vim.bo[bufnr].eol or (vim.bo[bufnr].fixeol and not vim.bo[bufnr].binary))
    --fix_eol = fix_eol and get_line(bufnr, max - 1) == ""
    --if fix_eol then
    --    vim.api.nvim_buf_set_lines(bufnr, -2, -1, false, {})
    --end
end

-- TEST SUITE

local function assertEqual(left, right)
    assert(left == right, "not equal: " .. tostring(left) .. " != " .. tostring(right))
end

local function assertFail(call)
    local status, _ = pcall(call)
    assert(not status, "Call did not fail, although it should have.")
end

function M.testAllUnits()
    assertEqual(2 + 2, 4)
    assertFail(function()
        error("This should fail.")
    end)
    print("Ethersync tests successful!")
end

return M
