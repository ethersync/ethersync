local function new()
    return {}
end

local function connect(conn, addr, port, callback)
    local tcp = vim.loop.new_tcp()
    tcp:connect(addr, port, function(err)
        if err then
            callback(err)
        else
            conn.tcp = tcp

            callback(nil)
        end
    end)
end

local function read(conn, callback)
    conn.tcp:read_start(function(err2, data)
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

local function send(conn, message)
    vim.schedule(function()
        local json = vim.fn.json_encode(message)
        conn.tcp:write(json)
        conn.tcp:write("\n")
    end)
end

return {
    new = new,
    connect = connect,
    send = send,
    read = read
}
