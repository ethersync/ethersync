---dlog is a module for writing debug logs.
---
---WARNING: This file is auto-generated, DO NOT MODIFY.
---
---Example usage:
---  local d = require("dlog").logger("my_logger")
---  d("Formatted lua string %s, number %d, etc", "test", 42)
---
---This will print "Formatted lua string test, number 42, etc"
---
---If debuglog plugin is not installed, all logs are no-op.
---Read more at https://github.com/smartpde/debuglog#shim
local has_debuglog, debuglog = pcall(require, "debuglog")

local M = {}

local function noop(_) end

---Returns the logger object if the debuglog plugin installed, or a
---no-op function otherwise.
---@param logger_name string the name of the logger
---@return fun(msg: string, ...): any logger function
function M.logger(logger_name)
    if has_debuglog then
        return debuglog.logger_for_shim_only(logger_name)
    end
    return noop
end

---Checks if the logger is enabled.
---@param logger_name string the name of the logger
---@return boolean enabled whether the logger is enabled
function M.is_enabled(logger_name)
    if has_debuglog then
        return debuglog.is_enabled(logger_name)
    end
    return false
end

return M
