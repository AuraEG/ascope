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
            ascope.notify("Selected host: " .. item.value, "info")
        end
    })
end, "Open SSH Host Picker")
