local M = {}

function M.appendNewline()
    print("Appending newline...")
    vim.cmd("normal! Go")
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
