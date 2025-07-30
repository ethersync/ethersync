-- SPDX-FileCopyrightText: 2024 blinry <mail@blinry.org>
-- SPDX-FileCopyrightText: 2024 zormit <nt4u@kpvn.de>
--
-- SPDX-License-Identifier: AGPL-3.0-or-later

local sync = require("vim.lsp.sync")
local utils = require("ethersync.utils")
local debug = require("ethersync.logging").debug

local M = {}

-- Used to note that changes to the buffer should be ignored, and not be sent out as deltas.
local ignore_edits = false

function M.get_all_lines_respecting_eol(buffer)
    local lines = vim.api.nvim_buf_get_lines(buffer, 0, -1, true)

    -- If eol is on, that's like a virtual empty line after the current lines.
    if vim.bo[buffer].eol then
        table.insert(lines, "")
    end

    return lines
end

local function is_empty(diff)
    return diff.text == ""
        and diff.range["start"].line == diff.range["end"].line
        and diff.range["start"].character == diff.range["end"].character
end

-- Subscribes the callback to changes for a given buffer id and reports with a delta.
--
-- The delta can be expected to be in the format as specified in the daemon-editor protocol.
function M.track_changes(buffer, callback)
    -- Used to remember the previous content of the buffer, so that we can
    -- calculate the difference between the previous and the current content.
    local prev_lines = M.get_all_lines_respecting_eol(buffer)

    vim.api.nvim_buf_attach(buffer, false, {
        on_lines = function(
            _the_literal_string_lines --[[@diagnostic disable-line]],
            _buffer_handle --[[@diagnostic disable-line]],
            _changedtick, --[[@diagnostic disable-line]]
            first_line,
            last_line,
            new_last_line
        )
            -- First, clear the "modified" option, so that the buffer is not displayed as dirty.
            -- Being modified doesn't have meaning for ethersync-ed files.
            vim.api.nvim_buf_set_option(buffer, "modified", false)

            -- Line counts that we get called with are zero-based.
            -- last_line and new_last_line are exclusive

            -- TODO: optimize with a cache
            local curr_lines = M.get_all_lines_respecting_eol(buffer)

            -- Special case: When deleting the entire content, when 'eol' is on, there
            -- will still be a "virtual line" after the current empty line: The file content will be "\n".
            -- So new_last_line should not be 0, but 1!
            if vim.bo[buffer].eol and #curr_lines == 2 and curr_lines[1] == "" and last_line == 1 then
                new_last_line = 1
            end

            debug({ first_line = first_line, last_line = last_line, new_last_line = new_last_line })

            -- Are we currently ignoring edits?
            if ignore_edits then
                prev_lines = curr_lines
                return
            end

            debug({ curr_lines = curr_lines, prev_lines = prev_lines })
            local diff = sync.compute_diff(prev_lines, curr_lines, first_line, last_line, new_last_line, "utf-32", "\n")
            -- line/character indices in diff are zero-based.
            debug({ diff = diff })

            -- Special case: If the entire content is deleted, undo the special treatment introduced in
            -- https://github.com/neovim/neovim/pull/29904. We think it's incorrect. :P
            if #curr_lines == 1 and curr_lines[1] == "" then
                diff.range["start"].line = 0
            end

            -- TODO: Simplify the solution?
            -- For example, pull tests into good variable names like "ends_with_newline".
            -- TODO: Update the following comment to describe the problem and the solution more clearly.

            -- Sometimes, Neovim deletes full lines by deleting the last line, plus an imaginary newline at the end. For example, to delete the second line, Neovim would delete from (line: 1, column: 0) to (line: 2, column 0).
            -- But, in the case of deleting the last line, what we expect in the rest of Ethersync is to delete the newline *before* the line.
            -- So let's change the deleted range to (line: 0, column: [last character of the first line]) to (line: 1, column: [last character of the second line]).

            if diff.range["end"].line == #prev_lines then
                -- Range spans to a line one after the visible buffer lines.
                if diff.range["start"].line == 0 then
                    -- The range starts on the first line, so we can't "shift the range backwards".
                    -- Instead, we just shorten the range by one character.
                    diff.range["end"].line = diff.range["end"].line - 1
                    diff.range["end"].character = vim.fn.strchars(prev_lines[#prev_lines])
                    if string.sub(diff.text, -1) == "\n" then
                        -- The replacement ends with a newline.
                        -- Drop it, because we shortened the range not to include the newline.
                        diff.text = string.sub(diff.text, 1, -2)
                    end
                else
                    -- The range doesn't start on the first line.
                    if diff.range["end"].character == 0 then
                        -- The range ends at the beginning of the line after the visible lines.
                        if diff.range["start"].character == 0 then
                            -- Operation applies to beginning of lines, that means it's possible to shift it back.
                            -- Modify edit, s.t. not the last \n, but the one before is replaced.
                            diff.range["start"].line = diff.range["start"].line - 1
                            diff.range["end"].line = diff.range["end"].line - 1
                            diff.range["start"].character = vim.fn.strchars(prev_lines[diff.range["start"].line + 1])
                            diff.range["end"].character = vim.fn.strchars(prev_lines[diff.range["end"].line + 1])
                        elseif string.sub(diff.text, -1) == "\n" then
                            -- The replacement ends with a newline.
                            -- Drop it, and shorten the range by one character.
                            diff.text = string.sub(diff.text, 1, -2)
                            diff.range["end"].line = diff.range["end"].line - 1
                            diff.range["end"].character = vim.fn.strchars(prev_lines[diff.range["end"].line + 1])
                        else
                            vim.api.nvim_err_writeln(
                                "[ethersync] We don't know how to handle this case for a deletion after the last visible line. Please file a bug."
                            )
                        end
                    else
                        vim.api.nvim_err_writeln(
                            "[ethersync] We think a delta ending inside the line after the visible ones cannot happen. Please file a bug."
                        )
                    end
                end
            else
                -- The range does not extend past the visible buffer lines (diff.range["end"].line < #prev_lines).
                -- We might still want to make the delta prettier.
                -- TODO: Integrate these cases in the above if branches somehow?
                if
                    diff.range["end"].character == 0
                    and string.sub(diff.text, 1, 1) == "\n"
                    and diff.range["start"].character == vim.fn.strchars(prev_lines[diff.range["start"].line + 1])
                    and diff.range["start"].line < diff.range["end"].line
                then
                    -- Range starts at the end of a line, and spans the newline after it, but also begins with a newline.
                    -- This newline is redundant, and leads to less-pretty diffs. Remove it.
                    diff.text = string.sub(diff.text, 2, -1)
                    diff.range["start"].line = diff.range["start"].line + 1
                    diff.range["start"].character = 0
                end

                if
                    diff.range["end"].character == 0
                    and string.sub(diff.text, -1) == "\n"
                    and diff.range["start"].line < diff.range["end"].line
                then
                    -- Range ends on the beginning of a line, but the replacement ends with a newline.
                    -- This newline is redundant, and leads to less-pretty diffs. Remove it.
                    diff.text = string.sub(diff.text, 1, -2)
                    diff.range["end"].line = diff.range["end"].line - 1
                    diff.range["end"].character = vim.fn.strchars(prev_lines[diff.range["end"].line + 1])
                end
            end

            prev_lines = curr_lines

            if is_empty(diff) then
                return
            end

            local delta = {
                {
                    range = diff.range,
                    replacement = diff.text,
                },
            }

            debug({ final_delta = delta })

            callback(delta)
        end,
    })
end

function M.apply_delta(buffer, delta)
    local text_edits = {}
    for _, replacement in ipairs(delta) do
        local text_edit = {
            range = replacement.range,
            newText = replacement.replacement,
        }
        table.insert(text_edits, text_edit)
    end

    ignore_edits = true
    utils.apply_text_edits(text_edits, buffer, "utf-32")
    ignore_edits = false
end

return M
