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

-- Smart cd (Directory Syncing) on Shutdown
ascope.on("on_shutdown", function()
    local current_pane = os.getenv("TMUX_PANE")
    local cwd = ascope.get_cwd()
    if current_pane and cwd and cwd ~= "" then
        ascope.exec_shell("tmux", {"send-keys", "-t", current_pane, "cd " .. cwd, "Enter"}, function() end)
    end
end)

ascope.register_key(key, function()
    local selection = ascope.get_selection()
    local path = selection and selection.path or ascope.get_cwd()
    if path == "" then return end
    
    ascope.open_modal({
        title = "🖥 Tmux Integration Toolkit",
        subtitle = path,
        show_input = false,
        fixed = true,
        width = 80,
        height = 14,
        tabs = { "Smart Control", "Clipboard Buffer" },
        items = {
            -- Smart Control Tab
            { label = "Open in Vertical Split", value = "vsplit", tab = "Smart Control", icon = "⇅" },
            { label = "Open in Horizontal Split", value = "hsplit", tab = "Smart Control", icon = "⇄" },
            { label = "Open in New Window", value = "new_window", tab = "Smart Control", icon = "🪟" },
            { label = "Send File to Neovim Pane", value = "send_nvim", tab = "Smart Control", icon = "📝" },

            -- Clipboard Buffer Tab
            { label = "Yank File Path", value = "yank_path", tab = "Clipboard Buffer", icon = "📋" },
            { label = "Yank File Content", value = "yank_content", tab = "Clipboard Buffer", icon = "📄" },
            { label = "Yank Filename", value = "yank_name", tab = "Clipboard Buffer", icon = "🪪" },
            { label = "Create File from Buffer", value = "create_from_buffer", tab = "Clipboard Buffer", icon = "📥" },
            { label = "Rename Selected to Buffer", value = "rename_to_buffer", tab = "Clipboard Buffer", icon = "✏️" },
        },
        on_select = function(item)
            if item.value == "vsplit" then
                local parent_dir = path
                local cmd_args = {"split-window", "-h"}
                if selection and selection.is_file then
                    parent_dir = path:match("^(.+)/[^/]+$") or path
                    table.insert(cmd_args, "-c")
                    table.insert(cmd_args, parent_dir)
                    table.insert(cmd_args, "nvim " .. selection.name)
                else
                    table.insert(cmd_args, "-c")
                    table.insert(cmd_args, path)
                end
                ascope.exec_shell("tmux", cmd_args, function() end)

            elseif item.value == "hsplit" then
                local parent_dir = path
                local cmd_args = {"split-window", "-v"}
                if selection and selection.is_file then
                    parent_dir = path:match("^(.+)/[^/]+$") or path
                    table.insert(cmd_args, "-c")
                    table.insert(cmd_args, parent_dir)
                    table.insert(cmd_args, "nvim " .. selection.name)
                else
                    table.insert(cmd_args, "-c")
                    table.insert(cmd_args, path)
                end
                ascope.exec_shell("tmux", cmd_args, function() end)

            elseif item.value == "new_window" then
                local parent_dir = path
                local cmd_args = {"new-window"}
                if selection and selection.is_file then
                    parent_dir = path:match("^(.+)/[^/]+$") or path
                    table.insert(cmd_args, "-c")
                    table.insert(cmd_args, parent_dir)
                    table.insert(cmd_args, "nvim " .. selection.name)
                else
                    table.insert(cmd_args, "-c")
                    table.insert(cmd_args, path)
                end
                ascope.exec_shell("tmux", cmd_args, function() end)

            elseif item.value == "send_nvim" then
                ascope.exec_shell("tmux", {"list-panes", "-F", "#{pane_id}\t#{pane_current_command}\t#{pane_active}"}, function(stdout, stderr, exit_code)
                    if exit_code ~= 0 then return end
                    local target_pane = nil
                    local current_pane = os.getenv("TMUX_PANE")
                    for line in stdout:gmatch("[^\r\n]+") do
                        local id, cmd, active = line:match("^(%S+)\t([^\t]+)\t(%d)$")
                        if id and id ~= current_pane and (cmd:match("nvim") or cmd:match("vim")) then
                            target_pane = id
                            break
                        end
                    end
                    if not target_pane then
                        for line in stdout:gmatch("[^\r\n]+") do
                            local id, cmd, active = line:match("^(%S+)\t([^\t]+)\t(%d)$")
                            if id and id ~= current_pane then
                                target_pane = id
                                break
                            end
                        end
                    end

                    if target_pane then
                        ascope.exec_shell("tmux", {"send-keys", "-t", target_pane, "Escape", ":e " .. path, "Enter"}, function()
                            ascope.notify("Sent file to pane " .. target_pane, "info")
                        end)
                    else
                        ascope.notify("No other panes in active window", "warn")
                    end
                end)

            elseif item.value == "yank_path" then
                ascope.exec_shell("tmux", {"set-buffer", path}, function()
                    ascope.notify("Copied path to tmux buffer", "info")
                end)

            elseif item.value == "yank_name" then
                local name = selection and selection.name or path:match("^.+/(.+)$") or path
                ascope.exec_shell("tmux", {"set-buffer", name}, function()
                    ascope.notify("Copied filename to tmux buffer", "info")
                end)

            elseif item.value == "yank_content" then
                if selection and selection.is_file then
                    ascope.exec_shell("tmux", {"load-buffer", path}, function(stdout, stderr, exit_code)
                        if exit_code == 0 then
                            ascope.notify("Copied file content to tmux buffer", "info")
                        else
                            ascope.notify("Failed to copy file content: " .. tostring(stderr), "error")
                        end
                    end)
                else
                    ascope.notify("Selected item is not a file", "warn")
                end

            elseif item.value == "create_from_buffer" then
                ascope.exec_shell("tmux", {"show-buffer"}, function(stdout, stderr, exit_code)
                    if exit_code ~= 0 then
                        ascope.notify("Failed to read tmux buffer", "error")
                        return
                    end
                    local content = stdout
                    local filename = "buffer_paste.txt"
                    local clean_line = content:gsub("[\r\n]", "")
                    if #clean_line > 0 and #clean_line < 100 and not clean_line:match("[/\\?%*]") then
                        filename = clean_line
                    end
                    
                    local file_path = ascope.get_cwd() .. "/" .. filename
                    local f = io.open(file_path, "w")
                    if f then
                        f:write(content)
                        f:close()
                        ascope.notify("Created file '" .. filename .. "' from tmux buffer", "info")
                        ascope.navigate(ascope.get_cwd())
                    else
                        ascope.notify("Failed to create file " .. filename, "error")
                    end
                end)

            elseif item.value == "rename_to_buffer" then
                ascope.exec_shell("tmux", {"show-buffer"}, function(stdout, stderr, exit_code)
                    if exit_code ~= 0 then return end
                    local new_name = stdout:gsub("[\r\n]", "")
                    if #new_name == 0 or #new_name > 255 or new_name:match("[/\\?%*]") then
                        ascope.notify("Invalid name in tmux buffer to rename", "warn")
                        return
                    end
                    if not selection then
                        ascope.notify("No file selected to rename", "warn")
                        return
                    end
                    
                    local old_path = selection.path
                    local parent_dir = old_path:match("^(.+)/[^/]+$") or ascope.get_cwd()
                    local new_path = parent_dir .. "/" .. new_name
                    local ok, err = os.rename(old_path, new_path)
                    if ok then
                        ascope.notify("Renamed to '" .. new_name .. "'", "info")
                        ascope.navigate(ascope.get_cwd())
                    else
                        ascope.notify("Rename failed: " .. tostring(err), "error")
                    end
                end)
            end
        end
    })
end, "Open Tmux Integration Toolkit")
