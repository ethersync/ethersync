local sync = require("vim.lsp.sync")
local utils = require("utils")
local debug = require("logging").debug

local M = {}

-- Used to note that changes to the buffer should be ignored, and not be sent out as deltas.
local ignore_edits = false

-- Subscribes the callback to changes for a given buffer id and reports with a delta.
--
-- The delta can be expected to be in the format as specified in the daemon-editor protocol.
function M.trackChanges(buffer, callback)
    -- Used to remember the previous content of the buffer, so that we can
    -- calculate the difference between the previous and the current content.
    local prev_lines = vim.api.nvim_buf_get_lines(buffer, 0, -1, true)

    vim.api.nvim_buf_attach(buffer, false, {
        on_lines = function(
            _the_literal_string_lines --[[@diagnostic disable-line]],
            _buffer_handle --[[@diagnostic disable-line]],
            _changedtick, --[[@diagnostic disable-line]]
            first_line,
            last_line,
            new_last_line
        )
            -- Line counts that we get called with are zero-based.
            -- last_line and new_last_line are exclusive

            debug({ first_line = first_line, last_line = last_line, new_last_line = new_last_line })
            -- TODO: optimize with a cache
            local curr_lines = vim.api.nvim_buf_get_lines(buffer, 0, -1, true)

            -- Are we currently ignoring edits?
            if ignore_edits then
                prev_lines = curr_lines
                return
            end

            debug({ curr_lines = curr_lines, prev_lines = prev_lines })
            local diff = sync.compute_diff(prev_lines, curr_lines, first_line, last_line, new_last_line, "utf-32", "\n")
            -- line/character indices in diff are zero-based.
            debug({ diff = diff })

            -- TODO: Simplify the solution?
            -- TODO: Update the following comment to describe the problem and the solution more clearly.

            -- Sometimes, Vim deletes full lines by deleting the last line, plus an imaginary newline at the end. For example, to delete the second line, Vim would delete from (line: 1, column: 0) to (line: 2, column 0).
            -- But, in the case of deleting the last line, what we expect in the rest of Ethersync is to delete the newline *before* the line.
            -- So let's change the deleted range to (line: 0, column: [last character of the first line]) to (line: 1, column: [last character of the second line]).

            if diff.range["end"].line == #prev_lines then
                -- Range spans to a line one after the visible buffer lines.
                if diff.range["start"].line == 0 then
                    -- The range starts on the first line, so we can't "shift the range backwards".
                    -- Instead, we just shorten the range by one character.
                    diff.range["end"].line = diff.range["end"].line - 1
                    diff.range["end"].character = vim.fn.strchars(prev_lines[#prev_lines])
                    if string.sub(diff.text, vim.fn.strchars(diff.text)) == "\n" then
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
                            diff.range["start"].character =
                                vim.fn.strchars(prev_lines[diff.range["start"].line + 1], false)
                            diff.range["end"].character = vim.fn.strchars(prev_lines[diff.range["end"].line + 1], false)
                        elseif string.sub(diff.text, vim.fn.strchars(diff.text)) == "\n" then
                            -- The replacement ends with a newline.
                            -- Drop it, and shorten the range by one character.
                            diff.text = string.sub(diff.text, 1, -2)
                            diff.range["end"].line = diff.range["end"].line - 1
                            diff.range["end"].character = vim.fn.strchars(prev_lines[diff.range["end"].line + 1], false)
                        else
                            vim.fn.echoerr(
                                "We don't know how to handle this case for a deletion after the last visible line. Please file a bug."
                            )
                        end
                    else
                        vim.fn.echoerr(
                            "We think a delta ending inside the line after the visible ones cannot happen. Please file a bug."
                        )
                    end
                end
            end

            prev_lines = curr_lines

            local delta = {
                {
                    range = {
                        anchor = diff.range.start,
                        head = diff.range["end"],
                    },
                    replacement = diff.text,
                },
            }

            debug({ final_delta = delta })

            callback(delta)
        end,
    })
end

function M.applyDelta(buffer, delta)
    local text_edits = {}
    for _, replacement in ipairs(delta) do
        local text_edit = {
            range = {
                start = replacement.range.anchor,
                ["end"] = replacement.range.head,
            },
            newText = replacement.replacement,
        }
        table.insert(text_edits, text_edit)
    end

    ignore_edits = true
    utils.apply_text_edits(text_edits, buffer, "utf-32")
    ignore_edits = false
end

return M
