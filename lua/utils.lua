local M = {}

function M.contentOfCurrentBuffer()
    local buffer = 0 -- Current buffer.
    local start = 0 -- First line.
    local end_ = -1 -- Last line.
    local strict_indexing = true -- Don't automatically clamp indices to be in a valid range.
    local lines = vim.api.nvim_buf_get_lines(buffer, start, end_, strict_indexing)
    return vim.fn.join(lines, "\n")
end

function M.byteOffsetToCharOffset(byteOffset)
    local content = M.contentOfCurrentBuffer()
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

function M.charOffsetToByteOffset(charOffset)
    local content = M.contentOfCurrentBuffer()
    if charOffset >= vim.fn.strchars(content) then
        -- TODO: When can this happen?
        return vim.fn.strlen(content)
    else
        return vim.fn.byteidxcomp(content, charOffset)
    end
end

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

function M.rowColToIndex(row, col)
    local byte = vim.fn.line2byte(row + 1) + col - 1
    return M.byteOffsetToCharOffset(byte)
end

return M
