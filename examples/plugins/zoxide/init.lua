local zoxide = {}

function zoxide.query()
    ascope.exec_shell("zoxide", {"query", "--list", "--score"}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("Zoxide query failed", "error")
            return
        end
        local items = {}
        for line in stdout:gmatch("[^\r\n]+") do
            local score, path = line:match("^%s*(%S+)%s+(.+)$")
            if score and path then
                table.insert(items, { label = "[" .. score .. "] " .. path, value = path })
            end
        end
        if #items == 0 then
            ascope.notify("No zoxide database entries found", "warn")
            return
        end
        ascope.open_modal({
            title = "󰆛 Zoxide — Jump to...",
            items = items,
            on_select = function(item, mode)
                ascope.navigate(item.value)
            end
        })
    end)
end

return zoxide
