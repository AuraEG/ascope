ascope.notify("Loading Docker plugin...", "info")

local key = "alt-d"
if ascope.config and ascope.config["docker"] and ascope.config["docker"].key_binding then
    key = ascope.config["docker"].key_binding
end

-- Clean up any mounted/extracted docker or volume file paths on shutdown
ascope.on("on_shutdown", function()
    os.execute("rm -rf /tmp/ascope-docker-* /tmp/ascope-vol-* 2>/dev/null")
end)

-- Check Docker daemon status on startup
ascope.exec_shell("docker", {"info"}, function(stdout, stderr, exit_code)
    if exit_code ~= 0 then
        ascope.notify("Docker daemon is not running or inaccessible!", "warn")
    else
        ascope.notify("Docker integration ready ✓", "info")
    end
end)

local function check_compose_status()
    local cwd = ascope.get_cwd()
    if not cwd then return end
    
    local f1 = io.open(cwd .. "/docker-compose.yml", "r")
    local f2 = io.open(cwd .. "/docker-compose.yaml", "r")
    local compose_file = nil
    
    if f1 then
        compose_file = "docker-compose.yml"
        f1:close()
    elseif f2 then
        compose_file = "docker-compose.yaml"
        f2:close()
    end

    if compose_file then
        ascope.exec_shell("docker", {"compose", "ps", "--format", "{{.Service}}\t{{.State}}"}, function(stdout, stderr, exit_code)
            if exit_code == 0 then
                local services = {}
                for line in stdout:gmatch("[^\r\n]+") do
                    local service, state = line:match("^(%S+)%s+(.+)$")
                    if service and state then
                        local indicator = "○"
                        if state:find("running") or state:find("Up") then
                            indicator = "●"
                        end
                        table.insert(services, indicator .. " " .. service .. " (" .. state .. ")")
                    end
                end
                if #services > 0 then
                    ascope.set_dashboard_info("docker_compose", "🐳 Docker Compose Services", services)
                else
                    ascope.set_dashboard_info("docker_compose", "🐳 Docker Compose Services", { "No active services" })
                end
            else
                ascope.remove_dashboard_info("docker_compose")
            end
        end)
    else
        ascope.remove_dashboard_info("docker_compose")
    end
end

-- Parse active compose services status when entering directories
ascope.on("on_enter", check_compose_status)
ascope.on("on_startup", check_compose_status)

-- Container extraction browser helper
local function browse_container(id, name)
    local mount_path = "/tmp/ascope-docker-" .. id
    ascope.notify("Extracting container filesystem to " .. mount_path .. "...", "info")
    
    ascope.exec_shell("mkdir", {"-p", mount_path}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("Failed to create temp directory", "error")
            return
        end
        
        -- Copy container root filesystem in background
        ascope.exec_shell("docker", {"cp", id .. ":/", mount_path}, function(cp_stdout, cp_stderr, cp_exit_code)
            if cp_exit_code == 0 then
                ascope.notify("Successfully extracted " .. name .. " ✓", "info")
                ascope.navigate(mount_path)
            else
                ascope.notify("Failed to extract filesystem: " .. tostring(cp_stderr), "error")
            end
        end)
    end)
end

-- Volume extraction browser helper
local function browse_volume(volume_name)
    local temp_path = "/tmp/ascope-vol-" .. volume_name
    ascope.notify("Extracting volume data to " .. temp_path .. "...", "info")
    
    ascope.exec_shell("mkdir", {"-p", temp_path}, function(stdout, stderr, exit_code)
        if exit_code ~= 0 then
            ascope.notify("Failed to create temp directory", "error")
            return
        end
        
        -- Run temporary helper alpine container to copy volume contents into /tmp
        ascope.exec_shell("docker", {
            "run", "--rm",
            "-v", volume_name .. ":/data",
            "-v", temp_path .. ":/backup",
            "alpine", "cp", "-a", "/data/.", "/backup"
        }, function(d_stdout, d_stderr, d_exit)
            if d_exit == 0 then
                ascope.notify("Successfully extracted volume contents ✓", "info")
                ascope.navigate(temp_path)
            else
                ascope.notify("Failed to read volume: " .. tostring(d_stderr), "error")
            end
        end)
    end)
end

-- Handle selection callback from the main picker modal
local function handle_docker_selection(item)
    if item.value == "loading" or item.value == "none" then return end

    if item.tab == "Containers" then
        local id = item.value
        local name = item.label:match("🐳%s*(%S+)") or id
        local is_running = item.label:find("🟢") ~= nil

        local con_actions = {
            { label = "🖥 Drop to Container Shell (Foreground)", value = "foreground_shell", icon = "🖥" },
            { label = "🐚 Open Shell in Tmux (New Window)", value = "tmux_shell", icon = "🐚" },
            { label = "📋 View Logs (Foreground)", value = "foreground_logs", icon = "📋" },
            { label = "📜 Stream Logs in Tmux", value = "tmux_logs", icon = "📜" },
            { label = "📂 Browse Filesystem (docker cp)", value = "browse", icon = "📂" }
        }

        if is_running then
            table.insert(con_actions, { label = "🛑 Stop Container", value = "stop", icon = "🛑" })
        else
            table.insert(con_actions, { label = "▶ Start Container", value = "start", icon = "▶" })
        end
        table.insert(con_actions, { label = "🗑 Remove Container", value = "remove", icon = "🗑" })

        ascope.open_modal({
            title = "⚡ Container: " .. name,
            subtitle = "Select Container Action",
            fixed = true,
            width = 80,
            height = 14,
            items = con_actions,
            on_select = function(act_item)
                if act_item.value == "foreground_shell" then
                    ascope.exec_interactive("docker", {"exec", "-it", id, "sh"})
                elseif act_item.value == "tmux_shell" then
                    local tmux_env = os.getenv("TMUX")
                    if not tmux_env or tmux_env == "" then
                        ascope.notify("Tmux environment not detected", "warn")
                        return
                    end
                    ascope.exec_shell("tmux", {"new-window", "-n", "docker:" .. name, "docker exec -it " .. id .. " sh"}, function(stdout, stderr, exit_code)
                        if exit_code == 0 then
                            ascope.notify("Opened shell in tmux window", "info")
                        else
                            ascope.notify("Failed to open shell: " .. tostring(stderr), "error")
                        end
                    end)
                elseif act_item.value == "foreground_logs" then
                    ascope.exec_interactive("docker", {"logs", "-f", id})
                elseif act_item.value == "tmux_logs" then
                    local tmux_env = os.getenv("TMUX")
                    if not tmux_env or tmux_env == "" then
                        ascope.notify("Tmux environment not detected", "warn")
                        return
                    end
                    ascope.exec_shell("tmux", {"new-window", "-n", "logs:" .. name, "docker logs -f " .. id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Streaming logs in tmux window", "info")
                        else
                            ascope.notify("Failed to stream logs: " .. tostring(stderr), "error")
                        end
                    end)
                elseif act_item.value == "browse" then
                    browse_container(id, name)
                elseif act_item.value == "stop" then
                    ascope.notify("Stopping container " .. name .. "...", "info")
                    ascope.exec_shell("docker", {"stop", id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Stopped container ✓", "info")
                        else
                            ascope.notify("Failed to stop container: " .. tostring(stderr), "error")
                        end
                    end)
                elseif act_item.value == "start" then
                    ascope.notify("Starting container " .. name .. "...", "info")
                    ascope.exec_shell("docker", {"start", id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Started container ✓", "info")
                        else
                            ascope.notify("Failed to start container: " .. tostring(stderr), "error")
                        end
                    end)
                elseif act_item.value == "remove" then
                    ascope.notify("Removing container " .. name .. "...", "info")
                    ascope.exec_shell("docker", {"rm", "-f", id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Removed container ✓", "info")
                        else
                            ascope.notify("Failed to remove container: " .. tostring(stderr), "error")
                        end
                    end)
                end
            end
        })

    elseif item.tab == "Images" then
        local img_id = item.value
        local img_name = item.label:match("📦%s*(%S+)") or img_id

        ascope.open_modal({
            title = "⚡ Image: " .. img_name,
            subtitle = "Select Image Action",
            fixed = true,
            width = 80,
            height = 11,
            items = {
                { label = "▶ Run Container from Image", value = "run", icon = "▶" },
                { label = "🔍 Inspect Image", value = "inspect", icon = "🔍" },
                { label = "🗑 Delete Image", value = "delete", icon = "🗑" }
            },
            on_select = function(act_item)
                if act_item.value == "run" then
                    local safe_name = img_name:gsub("[^%w%.%-]", "_") .. "-run"
                    ascope.notify("Running container " .. safe_name .. "...", "info")
                    ascope.exec_shell("docker", {"run", "-d", "--name", safe_name, img_id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Started container: " .. safe_name .. " ✓", "info")
                        else
                            ascope.notify("Failed to run container: " .. tostring(stderr), "error")
                        end
                    end)
                elseif act_item.value == "inspect" then
                    ascope.exec_shell("docker", {"inspect", img_id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            local inspect_path = "/tmp/ascope-docker-inspect-" .. img_id .. ".json"
                            local f = io.open(inspect_path, "w")
                            if f then
                                f:write(stdout)
                                f:close()
                                ascope.notify("Saved inspection to JSON. Opening...", "info")
                                ascope.exec_interactive("less", {inspect_path})
                            else
                                ascope.notify("Failed to write inspection file", "error")
                            end
                        else
                            ascope.notify("Failed to inspect image: " .. tostring(stderr), "error")
                        end
                    end)
                elseif act_item.value == "delete" then
                    ascope.notify("Deleting image " .. img_name .. "...", "info")
                    ascope.exec_shell("docker", {"rmi", img_id}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Deleted image ✓", "info")
                        else
                            ascope.notify("Failed to delete image: " .. tostring(stderr), "error")
                        end
                    end)
                end
            end
        })

    elseif item.tab == "Volumes" then
        local vol_name = item.value

        ascope.open_modal({
            title = "⚡ Volume: " .. vol_name,
            subtitle = "Select Volume Action",
            fixed = true,
            width = 80,
            height = 11,
            items = {
                { label = "📂 Browse Volume Content", value = "browse", icon = "📂" },
                { label = "🗑 Delete Volume", value = "delete", icon = "🗑" }
            },
            on_select = function(act_item)
                if act_item.value == "browse" then
                    browse_volume(vol_name)
                elseif act_item.value == "delete" then
                    ascope.notify("Deleting volume " .. vol_name .. "...", "info")
                    ascope.exec_shell("docker", {"volume", "rm", vol_name}, function(stdout, stderr, exit)
                        if exit == 0 then
                            ascope.notify("Deleted volume ✓", "info")
                        else
                            ascope.notify("Failed to delete volume: " .. tostring(stderr), "error")
                        end
                    end)
                end
            end
        })
    end
end

-- Key binding to toggle the main Docker Explorer modal
ascope.register_key(key, function()
    local containers = {}
    local images = {}
    local volumes = {}
    local completed = 0

    -- Open loading placeholder modal immediately to keep TUI highly responsive
    ascope.open_modal({
        title = "🐳 Docker Explorer",
        subtitle = "Loading docker data...",
        input_title = "Filter your docker",
        fixed = true,
        width = 95,
        height = 16,
        items = {
            { label = "Loading containers...", value = "loading", tab = "Containers" },
            { label = "Loading images...", value = "loading", tab = "Images" },
            { label = "Loading volumes...", value = "loading", tab = "Volumes" }
        }
    })

    local function check_done()
        completed = completed + 1
        if completed == 3 then
            if #containers == 0 then
                table.insert(containers, { label = "🔴 No containers found", value = "none", tab = "Containers" })
            end
            if #images == 0 then
                table.insert(images, { label = "📦 No images found", value = "none", tab = "Images" })
            end
            if #volumes == 0 then
                table.insert(volumes, { label = "💾 No volumes found", value = "none", tab = "Volumes" })
            end

            local items = {}
            for _, c in ipairs(containers) do table.insert(items, c) end
            for _, img in ipairs(images) do table.insert(items, img) end
            for _, vol in ipairs(volumes) do table.insert(items, vol) end

            ascope.open_modal({
                title = "🐳 Docker Explorer",
                subtitle = "Containers / Images / Volumes",
                input_title = "Filter your docker",
                tabs = { "Containers", "Images", "Volumes" },
                fixed = true,
                width = 95,
                height = 16,
                items = items,
                on_select = handle_docker_selection
            })
        end
    end

    -- 1. Fetch Containers
    ascope.exec_shell("docker", {"ps", "-a", "--format", "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}"}, function(stdout, stderr, exit_code)
        if exit_code == 0 then
            for line in stdout:gmatch("[^\r\n]+") do
                local id, name, img, status, ports = line:match("^(%S+)\t([^\t]+)\t([^\t]+)\t([^\t]+)\t(.*)$")
                if id then
                    local is_running = status:find("Up") ~= nil
                    local indicator = is_running and "🟢" or "🔴"
                    if ports == "" then ports = "none" end
                    
                    local label = string.format("%s  %-20s  %-30s  %-15s  %s", indicator, name, img, status, ports)
                    table.insert(containers, {
                        label = label,
                        value = id,
                        tab = "Containers"
                    })
                end
            end
        end
        check_done()
    end)

    -- 2. Fetch Images
    ascope.exec_shell("docker", {"images", "--format", "{{.Repository}}:{{.Tag}}\t{{.ID}}\t{{.Size}}"}, function(stdout, stderr, exit_code)
        if exit_code == 0 then
            for line in stdout:gmatch("[^\r\n]+") do
                local repo_tag, id, size = line:match("^([^\t]+)\t(%S+)\t(%S+)$")
                if repo_tag then
                    local label = string.format("📦  %-45s  (%-12s)  %s", repo_tag, id, size)
                    table.insert(images, {
                        label = label,
                        value = id,
                        tab = "Images"
                    })
                end
            end
        end
        check_done()
    end)

    -- 3. Fetch Volumes
    ascope.exec_shell("docker", {"volume", "ls", "--format", "{{.Name}}\t{{.Driver}}"}, function(stdout, stderr, exit_code)
        if exit_code == 0 then
            for line in stdout:gmatch("[^\r\n]+") do
                local name, driver = line:match("^([^\t]+)\t(%S+)$")
                if name then
                    local label = string.format("💾  %-60s  [%s]", name, driver)
                    table.insert(volumes, {
                        label = label,
                        value = name,
                        tab = "Volumes"
                    })
                end
            end
        end
        check_done()
    end)
end, "Open Docker Explorer")
