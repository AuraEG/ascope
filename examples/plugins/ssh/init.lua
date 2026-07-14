ascope.notify("Loading SSH plugin...", "info")

local key = "alt-s"
if ascope.config and ascope.config["ssh"] and ascope.config["ssh"].key_binding then
    key = ascope.config["ssh"].key_binding
end

local active_mounts = {}

local function update_ssh_dashboard()
    local list = {}
    local count = 0
    for host, path in pairs(active_mounts) do
        table.insert(list, host .. " ➔ " .. path)
        count = count + 1
    end
    if count > 0 then
        ascope.set_dashboard_info("ssh", "🌐 Active SSH Mounts", list)
    else
        ascope.remove_dashboard_info("ssh")
    end
end

local function unmount_host(host, path, cb)
    ascope.exec_shell("fusermount", {"-u", path}, function(stdout, stderr, exit_code)
        if exit_code == 0 then
            cb(true)
        else
            -- Try umount as a fallback
            ascope.exec_shell("umount", {path}, function(u_stdout, u_stderr, u_exit)
                if u_exit == 0 then
                    cb(true)
                else
                    cb(false, u_stderr)
                end
            end)
        end
    end)
end

local function parse_ssh_config()
    local home = os.getenv("HOME")
    local config_path = home .. "/.ssh/config"
    local file = io.open(config_path, "r")
    if not file then
        return {}
    end
    
    local hosts = {}
    local current_host = nil
    
    for line in file:lines() do
        -- Strip comments and leading/trailing whitespace
        local clean_line = line:gsub("#.*", ""):gsub("^%s+", ""):gsub("%s+$", "")
        if clean_line ~= "" then
            -- Match "Host <names>"
            local host_match = clean_line:match("^[Hh][Oo][Ss][Tt]%s+(.+)$")
            if host_match then
                -- Ignore wildcard Host *
                if host_match ~= "*" then
                    -- There can be multiple space-separated hosts
                    for host in host_match:gmatch("%S+") do
                        current_host = {
                            alias = host,
                            hostname = host,
                            user = nil,
                            port = nil
                        }
                        table.insert(hosts, current_host)
                    end
                else
                    current_host = nil
                end
            elseif current_host then
                -- Match "HostName <value>"
                local hostname_val = clean_line:match("^[Hh][Oo][Ss][Tt][Nn][Aa][Mm][Ee]%s+(%S+)$")
                if hostname_val then
                    current_host.hostname = hostname_val
                end
                
                -- Match "User <value>"
                local user_val = clean_line:match("^[Uu][Ss][Ee][Rr]%s+(%S+)$")
                if user_val then
                    current_host.user = user_val
                end
                
                -- Match "Port <value>"
                local port_val = clean_line:match("^[Pp][Oo][Rr][Tt]%s+(%S+)$")
                if port_val then
                    current_host.port = port_val
                end
            end
        end
    end
    file:close()
    return hosts
end

ascope.register_key(key, function()
    local hosts = parse_ssh_config()
    if #hosts == 0 then
        ascope.notify("No SSH hosts found in ~/.ssh/config", "warn")
        return
    end
    
    local items = {}
    for _, host in ipairs(hosts) do
        local label = "🌐 " .. host.alias
        if host.user then
            label = label .. " (" .. host.user .. "@" .. host.hostname .. ")"
        else
            label = label .. " (" .. host.hostname .. ")"
        end
        table.insert(items, { label = label, value = host.alias })
    end
    
    ascope.open_modal({
        title = "🌐 SSH Host Picker",
        subtitle = "Select SSH Host",
        fixed = true,
        width = 80,
        height = 14,
        items = items,
        on_select = function(item)
            local is_mounted = (active_mounts[item.value] ~= nil)
            local menu_items = {
                { label = "🐚 Open Remote Shell in Tmux", value = "shell", icon = "🐚" }
            }
            if is_mounted then
                table.insert(menu_items, 1, { label = "📤 Unmount Remote Filesystem", value = "unmount", icon = "📤" })
            else
                table.insert(menu_items, 1, { label = "📁 Mount Remote via SSHFS", value = "mount", icon = "📁" })
            end

            ascope.open_modal({
                title = "⚡ SSH Action: " .. item.value,
                subtitle = "Select Connection Mode",
                fixed = true,
                width = 80,
                height = 12,
                items = menu_items,
                on_select = function(act_item)
                    if act_item.value == "shell" then
                        local tmux_env = os.getenv("TMUX")
                        if not tmux_env or tmux_env == "" then
                            ascope.notify("Tmux environment not detected to open shell", "warn")
                            return
                        end
                        ascope.exec_shell("tmux", {"new-window", "-n", "ssh:" .. item.value, "ssh " .. item.value}, function(stdout, stderr, exit_code)
                            if exit_code == 0 then
                                ascope.notify("Opened remote SSH shell in tmux window", "info")
                            else
                                ascope.notify("Failed to open remote shell: " .. tostring(stderr), "error")
                            end
                        end)
                    elseif act_item.value == "mount" then
                        local mount_path = "/tmp/ascope-ssh-" .. item.value
                        ascope.exec_shell("mkdir", {"-p", mount_path}, function(stdout, stderr, exit_code)
                            if exit_code ~= 0 then
                                ascope.notify("Failed to create mount directory: " .. tostring(stderr), "error")
                                return
                            end
                            
                            ascope.notify("Mounting " .. item.value .. " to " .. mount_path .. "...", "info")
                            ascope.exec_shell("sshfs", {item.value .. ":/", mount_path}, function(m_stdout, m_stderr, m_exit_code)
                                if m_exit_code == 0 then
                                    active_mounts[item.value] = mount_path
                                    update_ssh_dashboard()
                                    ascope.notify("Successfully mounted remote filesystem ✓", "info")
                                    ascope.navigate(mount_path)
                                else
                                    ascope.notify("Failed to mount remote: " .. tostring(m_stderr), "error")
                                end
                            end)
                        end)
                    elseif act_item.value == "unmount" then
                        local path = active_mounts[item.value]
                        if path then
                            ascope.notify("Unmounting " .. item.value .. "...", "info")
                            unmount_host(item.value, path, function(success, err)
                                if success then
                                    active_mounts[item.value] = nil
                                    update_ssh_dashboard()
                                    ascope.notify("Successfully unmounted " .. item.value, "info")
                                    
                                    -- Navigate away if currently browsing the mounted folder
                                    local cwd = ascope.get_cwd()
                                    if cwd:find(path, 1, true) then
                                        ascope.navigate(os.getenv("HOME"))
                                    end
                                else
                                    ascope.notify("Unmount failed: " .. tostring(err), "error")
                                end
                            end)
                        end
                    end
                end
            })
        end
    })
end, "Open SSH Host Picker")
