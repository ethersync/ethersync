local M = {}

function M.contentOfCurrentBuffer()
    local buffer = 0 -- Current buffer.
    local start = 0 -- First line.
    local end_ = -1 -- Last line.
    local strict_indexing = true -- Don't automatically clamp indices to be in a valid range.
    local lines = vim.api.nvim_buf_get_lines(buffer, start, end_, strict_indexing)
    return vim.fn.join(lines, "\n")
end

-- Converts a UTF-8 byte offset to a Unicode character offset.
function M.byteOffsetToCharOffset(byteOffset, content)
    content = content or M.contentOfCurrentBuffer()

    local value = vim.fn.charidx(content, byteOffset, true)
    if value == -1 then
        -- In cases where the specified location is outside of the current content,
        -- we try to give a reasonable value, but (TODO) we don't actually know how many
        -- *characters* we need to add. For now, we use bytes.

        -- This case seems to trigger when deleting at the end of the file,
        -- and when using the 'o' command.
        local charLength = vim.fn.strchars(content)
        local byteLength = vim.fn.strlen(content)
        local bytesAfterContent = byteOffset - byteLength
        return charLength + bytesAfterContent
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
function M.charOffsetToByteOffset(charOffset)
    local content = M.contentOfCurrentBuffer()
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

function M.UTF16CodeUnitOffsetToCharOffset(utf16CodeUnitOffset)
    local content = M.contentOfCurrentBuffer()

    local pos = 0
    local UTF16CodeUnitsRemaining = utf16CodeUnitOffset

    while UTF16CodeUnitsRemaining > 0 do
        local char = vim.fn.strgetchar(content, pos)
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

return M
