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

-- Insert a string into the current buffer at a specified Unicode character index.
function M.insert(index, content)
    -- If 'eol' is on, there's an implied newline at the end of the buffer.
    -- If the index refers to a location *after* that newline, we also need to insert a newline before 'content'.
    -- In that case, decrement the index by 1, and set 'eol' off.
    if vim.api.nvim_get_option_value("eol", { buf = 0 }) then
        if index == vim.fn.strchars(M.contentOfCurrentBuffer()) then
            content = "\n" .. content
            index = index - 1
            vim.api.nvim_set_option_value("eol", false, { scope = "local" })
        end
    end

    local row, col = M.indexToRowCol(index)
    vim.api.nvim_buf_set_text(0, row, col, row, col, vim.split(content, "\n"))
end

-- Delete a string from the current buffer at a specified Unicode character index.
function M.delete(index, length)
    -- If 'eol' is on...
    if vim.api.nvim_get_option_value("eol", { buf = 0 }) then
        -- ...and this deletion would remove the (implied) newline at the end of the buffer...
        if index + length == vim.fn.strchars(M.contentOfCurrentBuffer()) then
            -- ...decrement the deleted length (because the newline isn't really there).
            length = length - 1

            -- If, after the deletion, there's no newline at the end of the buffer anymore
            -- (either because the deletion deleted everything, or because the last remaining
            -- character is not a newline)...
            local byteOffsetOfLastCharAfterDeletion = M.charOffsetToByteOffset(index - 1)
            local content = M.contentOfCurrentBuffer()
            if index == 0 or content[byteOffsetOfLastCharAfterDeletion] ~= "\n" then
                -- ...set 'eol' off.
                vim.api.nvim_set_option_value("eol", false, { scope = "local" })
            else
                -- Otherwise, leave it on, but also delete the trailing newline.
                index = index - 1
                length = length + 1
            end
        end
    end

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
    if vim.api.nvim_get_option_value("eol", { buf = buffer }) then
        -- vim has "consumed" an EOL and it's implicity.
        -- For the purpose of our buffer, we should keep track of the
        -- new line, which is not displayed.
        table.insert(lines, "")
    end
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
    local byteOffsetOfLine = vim.api.nvim_buf_get_offset(0, row)
    local col = byte - byteOffsetOfLine

    return row, col
end

-- Converts a row and column in the current buffer to a Unicode character offset.
function M.rowColToIndex(row, col)
    -- Note: line2byte returns 1 for the first line.
    local byte = vim.fn.line2byte(row) + col - 1
    return M.byteOffsetToCharOffset(byte)
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
    assertEqual(M.contentOfCurrentBuffer(), "x\n")
    vim.cmd("bd!")

    vim.cmd("enew")
    -- file did not contain a newline => eol will be false
    vim.api.nvim_set_option_value("eol", false, { scope = "local" })
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x" })
    assertEqual(M.contentOfCurrentBuffer(), "x")
    vim.cmd("bd!")

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "y" })
    assertEqual(M.contentOfCurrentBuffer(), "x\ny\n")
    vim.cmd("bd!")

    vim.cmd("enew")
    vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    assertEqual(M.contentOfCurrentBuffer(), "x\n\n")
    vim.cmd("bd!")

    -- vim.cmd("enew")
    -- vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    -- local row, col = M.indexToRowCol(2)
    -- assertEqual(row, 1)
    -- assertEqual(col, 0)

    -- vim.cmd("enew")
    -- vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    -- local row, col = M.indexToRowCol(1)
    -- assertEqual(row, 0)
    -- assertEqual(col, 1)

    -- vim.cmd("enew")
    -- vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    -- M.insert(2, "a")
    -- assertEqual(M.contentOfCurrentBuffer(), "x\na")

    -- vim.cmd("enew")
    -- vim.api.nvim_buf_set_text(0, 0, 0, 0, 0, { "x", "" })
    -- M.insert(1, "a")
    -- assertEqual(M.contentOfCurrentBuffer(), "xa\n")

    print("Ethersync tests successful!")
end

return M
