ascope.on("open_tmux_bookmark", function()
    ascope.exec_shell("tmux", {"list-sessions", "-F", "#{session_name}:#{session_path}"}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("No active Tmux sessions or tmux not running", "error")
            return
        end

        local items = {}
        for line in stdout:gmatch("[^\r\n]+") do
            local parts = {}
            for part in line:gmatch("[^:]+") do
                table.insert(parts, part)
            end
            if #parts >= 2 then
                local name = parts[1]
                local path = parts[2]
                table.insert(items, {
                    label = name .. " (" .. path .. ")",
                    value = path
                })
            elseif #parts == 1 then
                local name = parts[1]
                table.insert(items, {
                    label = name,
                    value = name
                })
            end
        end

        if #items == 0 then
            ascope.notify("No Tmux sessions found", "warn")
            return
        end

        ascope.open_modal({
            title = "Tmux Sessions",
            items = items,
            on_select = function(item, mode)
                ascope.navigate(item.value)
                ascope.notify("Navigated to Tmux session path: " .. item.value, "info")
            end
        })
    end)
end)
