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
            { label = "[p] Send path to pane...", value = "send_pane" },
            { label = "[s] Switch session...", value = "switch_session" },
        },
        on_select = function(item)
            if item.value == "vsplit" then
                ascope.exec_shell("tmux", {"split-window", "-h", "-c", path}, function() end)
            elseif item.value == "hsplit" then
                ascope.exec_shell("tmux", {"split-window", "-v", "-c", path}, function() end)
            elseif item.value == "window" then
                ascope.exec_shell("tmux", {"new-window", "-c", path}, function() end)
            elseif item.value == "send_pane" then
                ascope.exec_shell("tmux", {"list-panes", "-a", "-F", "#{pane_id}\t#{session_name}:#{window_name}\t#{pane_current_command}"}, function(stdout, stderr, exit_code)
                    if exit_code ~= 0 then
                        ascope.notify("Failed to query tmux panes", "error")
                        return
                    end
                    local panes = {}
                    for line in stdout:gmatch("[^\r\n]+") do
                        local id, info, cmd = line:match("^(%S+)\t([^\t]+)\t(.*)$")
                        if id then
                            table.insert(panes, { label = "󰞷 " .. info .. " (" .. cmd .. ")", value = id })
                        end
                    end
                    if #panes == 0 then
                        ascope.notify("No tmux panes found", "warn")
                        return
                    end
                    ascope.open_modal({
                        title = "󰞷 Send path to pane",
                        items = panes,
                        on_select = function(pane_item)
                            ascope.exec_shell("tmux", {"send-keys", "-t", pane_item.value, path, "Enter"}, function()
                                ascope.notify("Sent path to pane " .. pane_item.label, "info")
                            end)
                        end
                    })
                end)
            elseif item.value == "switch_session" then
                ascope.exec_shell("tmux", {"list-sessions", "-F", "#{session_name}\t#{session_windows} windows"}, function(stdout, stderr, exit_code)
                    if exit_code ~= 0 then
                        ascope.notify("Failed to query tmux sessions", "error")
                        return
                    end
                    local sessions = {}
                    for line in stdout:gmatch("[^\r\n]+") do
                        local name, info = line:match("^(%S+)\t(.*)$")
                        if name then
                            table.insert(sessions, { label = "🖥 " .. name .. " (" .. info .. ")", value = name })
                        end
                    end
                    if #sessions == 0 then
                        ascope.notify("No tmux sessions found", "warn")
                        return
                    end
                    ascope.open_modal({
                        title = "🖥 Switch Tmux Session",
                        items = sessions,
                        on_select = function(sess_item)
                            ascope.exec_shell("tmux", {"switch-client", "-t", sess_item.value}, function()
                                ascope.notify("Switched to session " .. sess_item.value, "info")
                            end)
                        end
                    })
                end)
            end
        end
    })
end, "Open Tmux Actions")
