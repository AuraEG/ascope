ascope.notify("Loading SSH plugin...", "info")

local key = "alt-s"
if ascope.config and ascope.config["ssh"] and ascope.config["ssh"].key_binding then
    key = ascope.config["ssh"].key_binding
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
            ascope.open_modal({
                title = "⚡ SSH Action: " .. item.value,
                subtitle = "Select Connection Mode",
                fixed = true,
                width = 80,
                height = 12,
                items = {
                    { label = "📁 Mount Remote via SSHFS", value = "mount", icon = "📁" },
                    { label = "🐚 Open Remote Shell in Tmux", value = "shell", icon = "🐚" },
                },
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
                        ascope.notify("Mounting not implemented yet", "info")
                    end
                end
            })
        end
    })
end, "Open SSH Host Picker")
