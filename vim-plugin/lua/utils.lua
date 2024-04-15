local M = {}

-- This file provides helper functions to convert between a number of ways to index a buffer:

-- * Byte offset: The number of UTF-8 bytes from the start of the buffer.
--   This is what Neovim uses internally.

-- * Character offset: The number of Unicode characters from the start of the buffer.
--   Neovim provides functions to output this, as well. These usually contain "char" in their name.

-- * UTF-16 code unit offset: The number of UTF-16 code units from the start of the buffer.
--   This is what Y.js uses internally. Neovim doesn't provide helper functions for this,
--   but we can iterate over the buffer content and calculate it ourselves.
--   Assumption: All Unicode codepoints under 0x10000 are encoded as a single UTF-16 code unit,
--   and all others as two.
--   https://en.wikipedia.org/wiki/UTF-16#Code_points_from_U+010000_to_U+10FFFF

-- Insert a string into the current buffer at a specified UTF-8 char index.
function M.insert(index, text)
    local row, col = M.indexToRowCol(index)
    vim.api.nvim_buf_set_text(0, row, col, row, col, vim.split(text, "\n"))
end

function M.appendNewline()
    print("Appending newline...")
    M.insert(vim.fn.strchars(M.contentOfCurrentBuffer()), "\n")
end

-- Delete a string from the current buffer at a specified UTF-16 code unit index.
function M.delete(index, length)
    local row, col = M.indexToRowCol(index)
    local rowEnd, colEnd = M.indexToRowCol(index + length)
    vim.api.nvim_buf_set_text(0, row, col, rowEnd, colEnd, { "" })
end

function M.contentOfCurrentBuffer()
    local buffer = 0 -- Current buffer.
    local start = 0 -- First line.
    local end_ = -1 -- Last line.
    local strict_indexing = true -- Don't automatically clamp indices to be in a valid range.
    local lines = vim.api.nvim_buf_get_lines(buffer, start, end_, strict_indexing)
    -- TODO: might be brittle to rely on \n as line delimiter?
    -- TODO: what happens if we open a latin-1 encoded file?
    return vim.fn.join(lines, "\n")
end

-- Converts a UTF-8 byte offset to a Unicode character offset.
function M.byteOffsetToCharOffset(byteOffset, content)
    content = content or M.contentOfCurrentBuffer()

    -- Special case: If the content is empty, looking up offset 0 should work.
    if content == "" and byteOffset == 0 then
        return 0
    end

    local value = vim.fn.charidx(content, byteOffset, true)
    if value == -1 then
        -- charidx returns -1 if we specify the byte position directly after the string,
        -- but we think that's a valid position.

        value = vim.fn.charidx(content, byteOffset - 1, true)
        if value ~= -1 then
            return value + 1
        else
            error("Could not look up byte offset " .. tostring(byteOffset) .. " in given content.")
        end
    end
    return value
end

-- Converts a Unicode character offset to a UTF-8 byte offset.
function M.charOffsetToByteOffset(charOffset, content)
    content = content or M.contentOfCurrentBuffer()
    if charOffset >= vim.fn.strchars(content) then
        -- TODO: When can this happen?
        return vim.fn.strlen(content)
    else
        return vim.fn.byteidxcomp(content, charOffset)
    end
end

-- Converts a Unicode character offset in the current buffer to a row and column.
function M.indexToRowCol(index)
    -- First, calculate which byte the (UTF-16) index corresponds to.
    local byte = M.charOffsetToByteOffset(index)

    local row = vim.fn.byte2line(byte + 1) - 1
    local col
    if row < 0 then
        -- This can happen if the requested index is at the end of the content.
        local content = M.contentOfCurrentBuffer()
        if content == "" then
            return 0, 0
        else
            if index == vim.fn.strchars(content) then
                -- There are two cases: If the last character is a newline, return the position in the next line.
                -- Otherwise, return the position after the line.
                row, col = M.indexToRowCol(index - 1)
                if string.sub(content, -1) == "\n" then
                    return row + 1, 0
                else
                    return row, col + 1
                end
            else
                error("Could not get row/col for index " .. tostring(index) .. " in content " .. content)
            end
        end
    else
        local byteOffsetOfLine = vim.api.nvim_buf_get_offset(0, row)
        col = byte - byteOffsetOfLine
    end

    return row, col
end

-- Converts a row and column in the current buffer to a Unicode character offset.
-- row starts from 1, col is a byte offset that starts from 0.
function M.rowColToIndex(row, col)
    -- Note: line2byte returns 1 for the first line.
    local byte = vim.fn.line2byte(row) + col - 1
    return M.byteOffsetToCharOffset(byte)
end

function M.rowColToIndexInLines(row, col, lines)
    local byte = 0
    for i = 1, row - 1 do
        byte = byte + vim.fn.strlen(lines[i]) + 1
    end
    byte = byte + col
    return M.byteOffsetToCharOffset(byte, table.concat(lines, "\n"))
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
    assertEqual(#vim.split("a\nb\n", "\n"), 3)

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x" })
    -- TODO: What we *really* expect here is "x\n".
    assertEqual(M.contentOfCurrentBuffer(), "x")

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "y" })
    assertEqual(M.contentOfCurrentBuffer(), "x\ny")

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    assertEqual(M.contentOfCurrentBuffer(), "x\n")

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    local row, col = M.indexToRowCol(2)
    assertEqual(row, 1)
    assertEqual(col, 0)

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    row, col = M.indexToRowCol(1)
    assertEqual(row, 0)
    assertEqual(col, 1)

    vim.cmd("enew")
    assertEqual(M.contentOfCurrentBuffer(), "")
    row, col = M.indexToRowCol(0)
    assertEqual(row, 0)
    assertEqual(col, 0)

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    M.insert(2, "a")
    assertEqual(M.contentOfCurrentBuffer(), "x\na")

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    M.insert(1, "a")
    assertEqual(M.contentOfCurrentBuffer(), "xa\n")

    assertEqual(M.charOffsetToByteOffset(0, ""), 0)

    vim.cmd("enew")
    M.appendNewline()

    assertEqual(M.byteOffsetToCharOffset(8, "x\ny\nzöz"), 7)

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "y", "zöz" })
    assertEqual(M.rowColToIndex(1, 0), 0)
    assertEqual(M.rowColToIndex(2, 0), 2)
    assertEqual(M.rowColToIndex(3, 4), 7)

    assertEqual(M.rowColToIndexInLines(1, 0, { "x", "y", "zöz" }), 0)
    assertEqual(M.rowColToIndexInLines(2, 0, { "x", "y", "zöz" }), 2)
    assertEqual(M.rowColToIndexInLines(3, 4, { "x", "y", "zöz" }), 7)

    print("Ethersync tests successful!")
end

return M
