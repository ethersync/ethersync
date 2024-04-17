local M = {}

function M.appendNewline()
    print("Appending newline...")
    vim.cmd("normal! Go")
end

function M.contentOfCurrentBuffer()
    local buffer = 0 -- Current buffer.
    local start = 0 -- First line.
    local end_ = -1 -- Last line.
    local strict_indexing = true -- Don't automatically clamp indices to be in a valid range.
    local lines = vim.api.nvim_buf_get_lines(buffer, start, end_, strict_indexing)
    -- TODO: might be brittle to rely on \n as line delimiter?
    -- TODO: what happens if we open a latin-1 encoded file?
    local result = vim.fn.join(lines, "\n")

    --if vim.bo.eol then
    --    result = result .. "\n"
    --end

    return result
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
    M.appendNewline()
    assertEqual(M.contentOfCurrentBuffer(), "\n")

    print("Ethersync tests successful!")
end

return M
