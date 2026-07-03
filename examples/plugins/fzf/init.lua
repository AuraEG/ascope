ascope_fzf = {}

function ascope_fzf.pick(title, items, on_select)
    ascope.open_modal({
        title = title,
        items = items,
        on_select = on_select
    })
end

local key = "ctrl-f"
if ascope.config and ascope.config["fzf"] and ascope.config["fzf"].key_binding then
    key = ascope.config["fzf"].key_binding
end

ascope.register_key(key, function()
    ascope.exec_shell("find", {".", "-type", "f"}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("File scanning failed", "error")
            return
        end
        local items = {}
        for line in stdout:gmatch("[^\r\n]+") do
            local cleaned = line:gsub("^%./", "")
            if cleaned ~= "" then
                table.insert(items, { label = "󰈔 " .. cleaned, value = cleaned })
            end
        end
        if #items == 0 then
            ascope.notify("No files found", "warn")
            return
        end
        ascope_fzf.pick("󰍉 Fzf — Fuzzy File Search", items, function(item, mode)
            ascope.navigate(item.value)
        end)
    end)
end)

return ascope_fzf
