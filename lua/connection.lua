local Connection = {}

function Connection:connect(addr, port, callback)
    self.tcp = vim.loop.new_tcp()
    self.tcp:connect(addr, port, function(err)
        if err then
            callback(err)
        else
            callback(nil)
        end
    end)
end

function Connection:read(callback)
    self.tcp:read_start(function(err2, data)
        if err2 then
            callback(err2, nil)
        else
            vim.schedule(function()
                local success, result = pcall(function() return vim.fn.json_decode(data) end)
                if success then
                    callback(nil, result)
                else
                    local error = result:gsub("^%s*(.-)%s*$", "%1")
                    callback(error, nil)
                end
            end)
        end
    end)
end

function Connection:send(message)
    vim.schedule(function()
        local json = vim.fn.json_encode(message)
        self.tcp:write(json)
        self.tcp:write("\n")
    end)
end

local M = {}

function M.new()
    return setmetatable({}, { __index = Connection })
end

return M
