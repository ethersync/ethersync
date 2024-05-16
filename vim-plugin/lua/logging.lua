local M = {}

M.log_file_handle = io.open("/tmp/ethersync-nvim.log", "a")

function M.debug(...)
    local objects = {}
    for i = 1, select("#", ...) do
        local v = select(i, ...)
        table.insert(objects, vim.inspect(v))
    end
    M.log_file_handle:write(table.concat(objects, "\n") .. "\n")
    M.log_file_handle:flush()
end

return M
