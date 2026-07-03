local key = "shift-z"
if ascope.config and ascope.config["zoxide"] and ascope.config["zoxide"].key_binding then
    key = ascope.config["zoxide"].key_binding
end

ascope.notify("Loading zoxide plugin with key: " .. key, "info")

local function zoxide_query()
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
                local actions = {
                    { label = "[c] Navigate here", value = "current" },
                    { label = "[t] Open in new tab", value = "tab" }
                }
                ascope.open_modal({
                    title = "Action: " .. item.value,
                    items = actions,
                    on_select = function(act, m)
                        if act.value == "tab" then
                            ascope.open_tab(item.value)
                        else
                            ascope.navigate(item.value)
                        end
                    end
                })
            end
        })
    end)
end

ascope.register_key(key, zoxide_query)

ascope.on("on_enter", function(path)
    ascope.exec_shell("zoxide", {"add", path}, function() end)
end)

ascope.notify("Zoxide plugin registered successfully", "info")
