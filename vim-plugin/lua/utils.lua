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

-- Converts a UTF-8 byte offset to a UTF-16 code unit offset.
-- TODO: Speed up?
function M.byteOffsetToUTF16CodeUnitOffset(byteOffset, content)
    content = content or M.contentOfCurrentBuffer()
    local charOffset = M.byteOffsetToCharOffset(byteOffset, content)
    return M.charOffsetToUTF16CodeUnitOffset(charOffset, content)
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

function M.UTF16CodeUnits(string)
    local chars = vim.fn.strchars(string)
    local pos = 0
    local utf16CodeUnitOffset = 0

    while pos < chars do
        local char = vim.fn.strgetchar(string, pos)
        if char < 0x10000 then
            utf16CodeUnitOffset = utf16CodeUnitOffset + 1
        else
            utf16CodeUnitOffset = utf16CodeUnitOffset + 2
        end
        pos = pos + 1
    end

    return utf16CodeUnitOffset
end

-- Converts a Unicode character offset to a UTF-16 code unit offset.
function M.charOffsetToUTF16CodeUnitOffset(charOffset, content)
    content = content or M.contentOfCurrentBuffer()

    if charOffset > vim.fn.strchars(content) then
        -- In cases where the specified location is outside of the current content,
        -- we try to give a reasonable value, but (TODO) we don't actually know how many
        -- *UTF-16 code units* we need to add. For now, we use bytes.

        -- This case seems to trigger when deleting at the end of the file,
        -- and when using the 'o' command.
        local charLength = vim.fn.strchars(content)
        local utf16Length = M.UTF16CodeUnits(content)
        local bytesAfterContent = charOffset - charLength
        return utf16Length + bytesAfterContent
    end

    return M.UTF16CodeUnits(vim.fn.strcharpart(content, 0, charOffset))
end

function M.UTF16CodeUnitOffsetToCharOffset(utf16CodeUnitOffset, content)
    content = content or M.contentOfCurrentBuffer()

    local pos = 0
    local UTF16CodeUnitsRemaining = utf16CodeUnitOffset

    while UTF16CodeUnitsRemaining > 0 do
        local char = vim.fn.strgetchar(content, pos)
        if char == -1 then
            error("Could not look up UTF-16 offset " .. tostring(utf16CodeUnitOffset) .. " in given content.")
        end
        if char < 0x10000 then
            UTF16CodeUnitsRemaining = UTF16CodeUnitsRemaining - 1
        else
            UTF16CodeUnitsRemaining = UTF16CodeUnitsRemaining - 2
        end
        pos = pos + 1
    end

    return pos
end

-- Converts a UTF-16 code unit offset to a row and column.
-- TODO: Speed up?
function M.UTF16CodeUnitOffsetToRowCol(utf16CodeUnitOffset)
    local charOffset = M.UTF16CodeUnitOffsetToCharOffset(utf16CodeUnitOffset)
    return M.indexToRowCol(charOffset)
end

-- Converts a Unicode character offset in the current buffer to a row and column.
function M.indexToRowCol(index)
    -- First, calculate which byte the (UTF-16) index corresponds to.
    local byte = M.charOffsetToByteOffset(index)

    -- Catch a special case: Querying the position after the last character.
    --local bufferLength = vim.fn.wordcount()["bytes"]
    --local afterLastChar = byte >= bufferLength
    --if afterLastChar then
    --    byte = bufferLength - 1
    --end

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

function assertEqual(left, right)
    assert(left == right, "not equal: " .. tostring(left) .. " != " .. tostring(right))
end

function assertFail(call)
    local status, err = pcall(call)
    assert(not status, "Call did not fail, although it should have.")
end

function M.testAllUnits()
    -- TODO: refactor: pull out tests per function
    assertEqual(M.UTF16CodeUnits("hello"), 5)
    -- 🥕 == U+1F955
    assertEqual(M.UTF16CodeUnits("🥕"), 2)

    -- utf8 1 byte, utf16 1 unit
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(4, "world"), 4)
    -- utf8 2 byte, utf16 1 unit
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(4, "äworld"), 3)
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(4, "ääworld"), 2)
    -- utf8 3 byte, utf16 1 unit
    -- ⚽ == U+26BD
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(4, "⚽world"), 2)
    -- utf8 3 byte, utf16 2 units
    -- does it exit? TODO
    -- utf8 4 byte, utf16 2 units
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(4, "🥕world"), 2)
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(5, "🥕world"), 3)
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(5, "world"), 5)
    assertFail(function()
        M.byteOffsetToUTF16CodeUnitOffset(6, "world")
    end)
    assertFail(function()
        M.byteOffsetToUTF16CodeUnitOffset(-1, "world")
    end)
    -- TODO: what happens if byteOffset doesn't match cleanly into a unit offset?
    -- TODO: handle more edge cases / catch more invalid input

    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(2, "world"), 2)
    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(2, "🥕world"), 1)
    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(4, "🥕world"), 3)
    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(5, "🥕wörld"), 4)
    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(4, "⚽world"), 4)

    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(5, "world"), 5)
    assertFail(function()
        M.UTF16CodeUnitOffsetToCharOffset(6, "world")
    end)

    assertEqual(M.byteOffsetToCharOffset(5, "world"), 5)
    assertFail(function()
        M.byteOffsetToCharOffset(6, "world")
    end)

    assertEqual(M.byteOffsetToCharOffset(0, ""), 0)
    assertEqual(M.UTF16CodeUnitOffsetToCharOffset(0, ""), 0)
    assertEqual(M.byteOffsetToUTF16CodeUnitOffset(0, "world"), 0)

    print("Ethersync tests successful!")
end

return M
