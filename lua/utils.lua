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
        -- TODO: This seems important for 'o' operations. But why?
        value = vim.fn.strchars(content)
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
    local col = byte - vim.api.nvim_buf_get_offset(0, row)

    return row, col
end

function M.rowColToIndex(row, col)
    local byte = vim.fn.line2byte(row + 1) + col - 1
    return M.byteOffsetToCharOffset(byte)
end

return M
