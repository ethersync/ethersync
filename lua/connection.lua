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
    local buffer = ""
    self.tcp:read_start(function(err2, chunk)
        if err2 then
            callback(err2, nil)
        else
            vim.schedule(function()
                buffer = buffer .. chunk
                while true do
                    -- For a complete message, we need a newline.
                    local start, _ = buffer:find("\n")

                    if start then
                        local json = buffer:sub(1, start - 1)
                        local success, result = pcall(function() return vim.fn.json_decode(json) end)
                        if success then
                            callback(nil, result)
                        else
                            -- Strip whitespace from error message.
                            local error = result:gsub("^%s*(.-)%s*$", "%1")
                            callback(error, nil)
                        end
                        buffer = buffer:sub(start + 1)
                    else
                        -- Message is incomplete, nothing left to parse at the moment.
                        break
                    end
                end
            end)
        end
    end)
end

function Connection:write(message)
    vim.schedule(function()
        local json = vim.fn.json_encode(message)
        self.tcp:write(json)
        self.tcp:write("\n")
    end)
end

local M = {}

function M.new_connection()
    return setmetatable({}, { __index = Connection })
end

return M
