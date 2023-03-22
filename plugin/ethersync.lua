ignored_ticks = {}

function EtherSync()
    print('Ethersync activated!')
    local buf = vim.api.nvim_get_current_buf()
    local row, col = unpack(vim.api.nvim_win_get_cursor(0))
    vim.api.nvim_buf_attach(0, false, {
        on_bytes = function(the_string_bytes, buffer_handle, changedtick, start_row, start_column, byte_offset, old_end_row, old_end_column, old_end_byte_length, new_end_row, new_end_column, new_end_byte_length)
            -- Did the change come from us? If so, ignore it.
            if ignored_ticks[changedtick] then
                ignored_ticks[changedtick] = nil
                return
            end

            print(start_column, old_end_column, new_end_column)
            local new_content_lines = vim.api.nvim_buf_get_text(buffer_handle, start_row, start_column, start_row+new_end_row, start_column+new_end_column, {})
            local changed_string = table.concat(new_content_lines, "\n")

            -- For testing, insert text one row below.
            vim.schedule(function()
                local nextTick = vim.api.nvim_buf_get_changedtick(buffer_handle)
                ignored_ticks[nextTick] = true
                vim.api.nvim_buf_set_text(buffer_handle, start_row+1, start_column, start_row+1, start_column+old_end_column, {changed_string})
            end)
        end
    })
end

-- when new buffer is loaded, run EtherSync
vim.api.nvim_exec([[
augroup EtherSync
    autocmd!
    autocmd BufEnter * lua EtherSync()
augroup END
]], false)

vim.api.nvim_create_user_command('EtherSync', EtherSync, {})
vim.keymap.set('n', '<Leader>p', EtherSync)
