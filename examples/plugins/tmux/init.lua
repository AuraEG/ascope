ascope.notify("Loading tmux plugin...", "info")

local key = "alt-t"
if ascope.config and ascope.config["tmux"] and ascope.config["tmux"].key_binding then
    key = ascope.config["tmux"].key_binding
end

local tmux_env = os.getenv("TMUX")
if tmux_env and tmux_env ~= "" then
    ascope.notify("Tmux detected ✓", "info")
else
    ascope.notify("Tmux environment not detected", "warn")
end

ascope.register_key(key, function()
    local selection = ascope.get_selection()
    local path = selection and selection.path or ascope.get_cwd()
    if path == "" then return end
    
    ascope.open_modal({
        title = "🖥 Tmux splits & windows",
        items = {
            { label = "[v] Open in vertical split", value = "vsplit" },
            { label = "[h] Open in horizontal split", value = "hsplit" },
            { label = "[w] Open in new window", value = "window" },
        },
        on_select = function(item)
            if item.value == "vsplit" then
                ascope.exec_shell("tmux", {"split-window", "-h", "-c", path}, function() end)
            elseif item.value == "hsplit" then
                ascope.exec_shell("tmux", {"split-window", "-v", "-c", path}, function() end)
            elseif item.value == "window" then
                ascope.exec_shell("tmux", {"new-window", "-c", path}, function() end)
            end
        end
    })
end, "Open Tmux Actions")
