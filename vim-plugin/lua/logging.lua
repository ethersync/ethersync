local M = {}

local logfile = os.getenv("ETHERSYNC_NVIM_LOGFILE")
if logfile then
    M._log_file_handle = io.open(logfile, "a")
end

function M.debug(value)
    if not M._log_file_handle then
        return
    end

    if type(value) ~= "string" then
        value = vim.inspect(value)
    end

    local date = os.date("%Y-%m-%d %H:%M:%S")
    local debug_info = debug.getinfo(2)

    local name = debug_info.name or "?"
    local line = debug_info.currentline or "?"
    local context = " " .. name .. ":" .. line

    M._log_file_handle:write(date .. context .. ": " .. value .. "\n")
    M._log_file_handle:flush()
end

return M
