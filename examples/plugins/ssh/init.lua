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

-- Clean unmount on shutdown
ascope.on("on_shutdown", function()
    for host, path in pairs(active_mounts) do
        os.execute("fusermount -u " .. path .. " 2>/dev/null || umount " .. path .. " 2>/dev/null")
    end
end)

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

local function start_add_host_wizard()
    ascope.open_modal({
        title = "✚ Step 1: Enter Host Alias (Name)",
        subtitle = "e.g., oracle-vm-1",
        input_title = "Enter Alias",
        show_input = true,
        fixed = true,
        width = 80,
        height = 12,
        items = {
            { label = "Type name in input bar above, then press Enter to submit...", value = "submit_alias" }
        },
        on_select = function(alias_item)
            local alias = alias_item.input:gsub("^%s+", ""):gsub("%s+$", "")
            if alias == "" or alias == "submit_alias" then
                ascope.notify("Invalid or empty host alias", "warn")
                return
            end
            
            ascope.open_modal({
                title = "✚ Step 2: Enter IP / Hostname for " .. alias,
                subtitle = "e.g., 129.146.2.8",
                input_title = "Enter HostName / IP",
                show_input = true,
                fixed = true,
                width = 80,
                height = 12,
                items = {
                    { label = "Type IP/hostname in input bar, then press Enter to submit...", value = "submit_ip" }
                },
                on_select = function(ip_item)
                    local hostname = ip_item.input:gsub("^%s+", ""):gsub("%s+$", "")
                    if hostname == "" or hostname == "submit_ip" then
                        ascope.notify("Invalid or empty IP/hostname", "warn")
                        return
                    end
                    
                    ascope.open_modal({
                        title = "✚ Step 3: Enter SSH User",
                        subtitle = "Press Enter to default to 'root'",
                        input_title = "Enter SSH User",
                        show_input = true,
                        fixed = true,
                        width = 80,
                        height = 12,
                        items = {
                            { label = "Type user in input bar, or press Enter to default to root...", value = "submit_user" }
                        },
                        on_select = function(user_item)
                            local user = user_item.input:gsub("^%s+", ""):gsub("%s+$", "")
                            if user == "" or user == "submit_user" then
                                user = "root"
                            end
                            
                            ascope.open_modal({
                                title = "✚ Step 4: Enter SSH Port",
                                subtitle = "Press Enter to default to '22'",
                                input_title = "Enter SSH Port",
                                show_input = true,
                                fixed = true,
                                width = 80,
                                height = 12,
                                items = {
                                    { label = "Type port in input bar, or press Enter to default to 22...", value = "submit_port" }
                                },
                                on_select = function(port_item)
                                    local port = port_item.input:gsub("^%s+", ""):gsub("%s+$", "")
                                    if port == "" or port == "submit_port" then
                                        port = "22"
                                    end
                                    
                                    ascope.open_modal({
                                        title = "✚ Step 5: Enter Private Key Path (Optional)",
                                        subtitle = "e.g., ~/cloverlabs-2 (Press Enter to skip)",
                                        input_title = "Enter Key Path",
                                        show_input = true,
                                        fixed = true,
                                        width = 80,
                                        height = 12,
                                        items = {
                                            { label = "Type key path in input bar, or press Enter to skip...", value = "submit_key" }
                                        },
                                        on_select = function(key_item)
                                            local identity = key_item.input:gsub("^%s+", ""):gsub("%s+$", "")
                                            if identity == "submit_key" then
                                                identity = ""
                                            end
                                            
                                            local home = os.getenv("HOME")
                                            local config_path = home .. "/.ssh/config"
                                            local f = io.open(config_path, "a")
                                            if f then
                                                f:write("\nHost " .. alias .. "\n")
                                                f:write("    HostName " .. hostname .. "\n")
                                                f:write("    User " .. user .. "\n")
                                                f:write("    Port " .. port .. "\n")
                                                if identity ~= "" then
                                                    f:write("    IdentityFile " .. identity .. "\n")
                                                end
                                                f:close()
                                                ascope.notify("Successfully added host '" .. alias .. "' to SSH config ✓", "info")
                                            else
                                                ascope.notify("Failed to write to ~/.ssh/config", "error")
                                            end
                                        end
                                    })
                                end
                            })
                        end
                    })
                end
            })
        end
    })
end

local function handle_adhoc_mount(adhoc_input)
    local target = adhoc_input:gsub("%s*-p%s+%d+", ""):gsub("^%s+", ""):gsub("%s+$", "")
    local port = adhoc_input:match("-p%s+(%d+)")
    
    if target == "" or target == "submit_adhoc" then
        ascope.notify("Invalid ad-hoc host target", "warn")
        return
    end

    -- Use clean name for folder
    local safe_name = target:gsub("[^%w%.%-]", "_")
    local mount_path = "/tmp/ascope-ssh-" .. safe_name
    
    ascope.exec_shell("mkdir", {"-p", mount_path}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("Failed to create mount folder", "error")
            return
        end

        local sshfs_args = {}
        if port then
            table.insert(sshfs_args, "-p")
            table.insert(sshfs_args, port)
        end
        table.insert(sshfs_args, target .. ":/")
        table.insert(sshfs_args, mount_path)

        ascope.notify("Mounting ad-hoc target to " .. mount_path .. "...", "info")
        ascope.exec_shell("sshfs", sshfs_args, function(m_stdout, m_stderr, m_exit_code)
            if m_exit_code == 0 then
                active_mounts[target] = mount_path
                update_ssh_dashboard()
                ascope.notify("Ad-hoc target mounted ✓", "info")
                
                ascope.open_modal({
                    title = "📂 Mount: " .. target,
                    subtitle = "Select Navigation Mode",
                    fixed = true,
                    width = 80,
                    height = 12,
                    items = {
                        { label = "[c] Navigate here on current tab", value = "current", icon = "📁" },
                        { label = "[t] Open in a new tab", value = "tab", icon = "🗂" },
                    },
                    on_select = function(nav_item)
                        if nav_item.value == "tab" then
                            ascope.open_tab(mount_path)
                        else
                            ascope.navigate(mount_path)
                        end
                    end
                })
            else
                local errMsg = tostring(m_stderr)
                if errMsg:find("os error 2") then
                    errMsg = "'sshfs' is not installed. Run: sudo apt install sshfs"
                end
                ascope.notify("Failed to mount ad-hoc: " .. errMsg, "error")
            end
        end)
    end)
end

local function start_adhoc_wizard()
    ascope.open_modal({
        title = "🔌 Connect to Ad-Hoc SSH Target",
        subtitle = "Format: user@ip [-p port]",
        input_title = "user@ip [-p port]",
        show_input = true,
        fixed = true,
        width = 80,
        height = 12,
        items = {
            { label = "Type target details and press Enter to connect...", value = "submit_adhoc" }
        },
        on_select = function(adhoc_item)
            local clean_input = adhoc_item.input:gsub("^%s+", ""):gsub("%s+$", "")
            if clean_input == "" or clean_input == "submit_adhoc" then
                ascope.notify("Input target cannot be empty", "warn")
                return
            end

            ascope.open_modal({
                title = "⚡ SSH Ad-Hoc: " .. clean_input,
                subtitle = "Select Connection Mode",
                fixed = true,
                width = 80,
                height = 13,
                items = {
                    { label = "📁 Mount Remote via SSHFS", value = "mount", icon = "📁" },
                    { label = "🖥 Drop to Remote Shell (Foreground)", value = "foreground_shell", icon = "🖥" },
                    { label = "🐚 Open Remote Shell in Tmux", value = "shell", icon = "🐚" },
                },
                on_select = function(act_item)
                    local target = clean_input:gsub("%s*-p%s+%d+", ""):gsub("^%s+", ""):gsub("%s+$", "")
                    local port = clean_input:match("-p%s+(%d+)")

                    if act_item.value == "shell" then
                        local tmux_env = os.getenv("TMUX")
                        if not tmux_env or tmux_env == "" then
                            ascope.notify("Tmux environment not detected to open shell", "warn")
                            return
                        end
                        local shell_args = {"new-window", "-n", "ssh:adhoc"}
                        if port then
                            table.insert(shell_args, "ssh " .. target .. " -p " .. port)
                        else
                            table.insert(shell_args, "ssh " .. target)
                        end
                        ascope.exec_shell("tmux", shell_args, function(stdout, stderr, exit_code)
                            if exit_code == 0 then
                                ascope.notify("Opened remote SSH shell in tmux window", "info")
                            else
                                ascope.notify("Failed to open remote shell: " .. tostring(stderr), "error")
                            end
                        end)
                    elseif act_item.value == "foreground_shell" then
                        local ssh_args = {}
                        if port then
                            table.insert(ssh_args, "-p")
                            table.insert(ssh_args, port)
                        end
                        table.insert(ssh_args, target)
                        ascope.exec_interactive("ssh", ssh_args)
                    elseif act_item.value == "mount" then
                        handle_adhoc_mount(clean_input)
                    end
                end
            })
        end
    })
end

ascope.register_key(key, function()
    local hosts = parse_ssh_config()
    local items = {}
    table.insert(items, { label = "✚ Add New SSH Host...", value = "add_host", icon = "✚" })
    table.insert(items, { label = "🔌 Connect to Ad-Hoc Host...", value = "adhoc", icon = "🔌" })
    
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
        input_title = "Filter your SSH",
        fixed = true,
        width = 80,
        height = 14,
        items = items,
        on_select = function(item)
            if item.value == "add_host" then
                start_add_host_wizard()
                return
            elseif item.value == "adhoc" then
                start_adhoc_wizard()
                return
            end

            local is_mounted = (active_mounts[item.value] ~= nil)
            local menu_items = {
                { label = "🖥 Drop to Remote Shell (Foreground)", value = "foreground_shell", icon = "🖥" },
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
                height = 13,
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
                    elseif act_item.value == "foreground_shell" then
                        ascope.exec_interactive("ssh", {item.value})
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
                                    
                                    ascope.open_modal({
                                        title = "📂 Mount: " .. item.value,
                                        subtitle = "Select Navigation Mode",
                                        fixed = true,
                                        width = 80,
                                        height = 12,
                                        items = {
                                            { label = "[c] Navigate here on current tab", value = "current", icon = "📁" },
                                            { label = "[t] Open in a new tab", value = "tab", icon = "🗂" },
                                        },
                                        on_select = function(nav_item)
                                            if nav_item.value == "tab" then
                                                ascope.open_tab(mount_path)
                                            else
                                                ascope.navigate(mount_path)
                                            end
                                        end
                                    })
                                else
                                    local errMsg = tostring(m_stderr)
                                    if errMsg:find("os error 2") then
                                        errMsg = "'sshfs' command is not installed on your system. Please install it using: sudo apt install sshfs"
                                    end
                                    ascope.notify("Failed to mount remote: " .. errMsg, "error")
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
