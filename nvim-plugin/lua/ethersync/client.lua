local M = {}

local client

function M.is_connected()
    return client ~= nil
end

-- Connect to the daemon.
function M.connect(directory, on_notification)
    if client then
        client.terminate()
    end

    -- TODO: executable should now be configurable
    if vim.fn.executable("ethersync") == 0 then
        vim.api.nvim_err_writeln(
            "Tried to connect to the Ethersync daemon, but `ethersync` executable was not found. Make sure that is in your PATH."
        )
        return false
    end

    local params = { "client", "--directory", directory }

    local dispatchers = {
        notification = on_notification,
        on_error = function(code, ...)
            print("Ethersync client connection error: ", code, vim.inspect({ ... }))
        end,
        on_exit = function(code, _)
            if code == 0 then
                vim.schedule(function()
                    vim.api.nvim_err_writeln(
                        "Connection to Ethersync daemon lost. Probably it crashed or was stopped. Please restart the daemon, then Neovim."
                    )
                    -- TODO: Enable writing here again, so that user can make backup of file?
                end)
            else
                print(
                    "Could not connect to Ethersync daemon. Did you start it (in "
                        .. directory
                        .. ")? To stop trying, remove the .ethersync/ directory."
                )
            end
        end,
    }

    if vim.version().api_level < 12 then
        -- In Neovim 0.9, the API was to pass the command and its parameters as two arguments.
        ---@diagnostic disable-next-line: param-type-mismatch
        client = vim.lsp.rpc.start("ethersync", params, dispatchers)
    else
        -- While in Neovim 0.10, it is combined into one table.
        local cmd = params
        table.insert(cmd, 1, "ethersync")
        client = vim.lsp.rpc.start(cmd, dispatchers)
    end

    print("Connected to Ethersync daemon!")
    return true
end

-- Pulled out as a method in case we want to add a new "offline simulation" later.
function M.send_notification(method, params)
    client.notify(method, params)
end

function M.send_request(method, params, result_callback, err_callback)
    err_callback = err_callback or function() end
    result_callback = result_callback or function() end

    client.request(method, params, function(err, result)
        if err then
            local error_msg = "[ethersync] Error for '" .. method .. "': " .. err.message
            if err.data and err.data ~= "" then
                error_msg = error_msg .. " (" .. err.data .. ")"
            end
            vim.api.nvim_err_writeln(error_msg)
            err_callback(err)
        end
        if result then
            result_callback(result)
        end
    end)
end

return M