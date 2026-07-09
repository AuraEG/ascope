ascope_fzf = {}

local function get_parent_dir(path)
    if path == "." or path == "" then
        return "."
    end
    -- Remove trailing slash
    if path:sub(-1) == "/" then
        path = path:sub(1, -2)
    end
    local parent = path:match("(.+)/[^/]+$")
    if not parent or parent == "" then
        if path:sub(1, 1) == "/" then
            return "/"
        else
            return "."
        end
    end
    return parent
end

local function is_media_file(filename)
    local ext = filename:match("^.+(%..+)$")
    if ext then
        ext = ext:lower()
        if ext == ".png" or ext == ".jpg" or ext == ".jpeg" or ext == ".gif" or ext == ".bmp" or ext == ".pdf" or ext == ".mp4" or ext == ".mkv" or ext == ".avi" or ext == ".mov" or ext == ".webm" then
            return true
        end
    end
    return false
end

function ascope_fzf.pick(title, items, on_select)
    ascope.open_modal({
        title = title,
        items = items,
        on_select = on_select
    })
end

ascope.fzf = ascope_fzf

local key = "shift-f"
if ascope.config and ascope.config["fzf"] and ascope.config["fzf"].key_binding then
    key = ascope.config["fzf"].key_binding
end

ascope.register_key(key, function()
    local initial_cwd = ascope.get_cwd()
    if initial_cwd == "" then
        initial_cwd = "."
    end
    local picker_cwd = initial_cwd

    local function load_dir(cwd)
        local is_initial = (cwd == initial_cwd)

        local function show_picker(items)
            ascope_fzf.pick("󰍉 Fzf — Fuzzy File Search", items, function(item, mode)
                if item.value == ".." then
                    picker_cwd = get_parent_dir(picker_cwd)
                    load_dir(picker_cwd)
                elseif item.value:sub(-1) == "/" then
                    -- Navigate into the absolute directory path directly
                    picker_cwd = item.value:sub(1, -2)
                    load_dir(picker_cwd)
                else
                    -- Open file!
                    local filepath = item.value
                    if is_media_file(filepath) then
                        ascope.open_in_default_app(filepath)
                    else
                        ascope.open_in_editor(filepath)
                    end
                    ascope.close_modal()
                end
            end)
        end

        -- Attempt Git command first for speed and smart ignore listing
        ascope.exec_shell("git", {"-c", "core.quotePath=false", "-C", cwd, "ls-files", "--cached", "--others", "--exclude-standard", "."}, function(stdout, stderr, exit_code)
            local files = {}
            local dirs = {}
            local dir_set = {}
            local count = 0

            if exit_code == 0 and stdout ~= "" then
                for line in stdout:gmatch("[^\r\n]+") do
                    if count >= 10000 then break end
                    if line ~= "" then
                        table.insert(files, line)
                        local first_slash = line:find("/")
                        if first_slash then
                            local first_dir = line:sub(1, first_slash)
                            if not dir_set[first_dir] then
                                dir_set[first_dir] = true
                                table.insert(dirs, first_dir)
                            end
                        end
                        count = count + 1
                    end
                end
            end

            if #files == 0 then
                -- Fallback to standard recursive find with depth limit, excluding hidden dirs, node_modules, target, build, dist folders and common binary extensions
                ascope.exec_shell("find", {
                    cwd,
                    "-maxdepth", "5",
                    "-type", "f",
                    "-not", "-path", "*/.*",
                    "-not", "-path", "*/node_modules/*",
                    "-not", "-path", "*/target/*",
                    "-not", "-path", "*/build/*",
                    "-not", "-path", "*/dist/*",
                    "-not", "-name", "*.o",
                    "-not", "-name", "*.a",
                    "-not", "-name", "*.so",
                    "-not", "-name", "*.dylib",
                    "-not", "-name", "*.class",
                    "-not", "-name", "*.pyc",
                    "-not", "-name", "*.out"
                }, function(find_stdout, find_stderr, find_exit_code)
                    local find_count = 0
                    for line in find_stdout:gmatch("[^\r\n]+") do
                        if find_count >= 10000 then break end
                        local cleaned = line
                        if line:sub(1, #cwd) == cwd then
                            cleaned = line:sub(#cwd + 2)
                        end
                        if cleaned ~= "" then
                            table.insert(files, cleaned)
                            local first_slash = cleaned:find("/")
                            if first_slash then
                                local first_dir = cleaned:sub(1, first_slash)
                                if not dir_set[first_dir] then
                                    dir_set[first_dir] = true
                                    table.insert(dirs, first_dir)
                                end
                            end
                            find_count = find_count + 1
                        end
                    end

                    -- Build items
                    local items = {}
                    if not is_initial then
                        table.insert(items, { label = "󱏒 ..", value = ".." })
                    end
                    table.sort(dirs)
                    for _, dir in ipairs(dirs) do
                        table.insert(items, { label = dir, value = cwd .. "/" .. dir })
                    end
                    table.sort(files)
                    for _, file in ipairs(files) do
                        table.insert(items, { label = file, value = cwd .. "/" .. file })
                    end
                    show_picker(items)
                end)
            else
                -- Build items
                local items = {}
                if not is_initial then
                    table.insert(items, { label = "󱏒 ..", value = ".." })
                end
                table.sort(dirs)
                for _, dir in ipairs(dirs) do
                    table.insert(items, { label = dir, value = cwd .. "/" .. dir })
                end
                table.sort(files)
                for _, file in ipairs(files) do
                    table.insert(items, { label = file, value = cwd .. "/" .. file })
                end
                show_picker(items)
            end
        end)
    end

    load_dir(picker_cwd)
end, "Open Fzf Picker (Fuzzy File Finder)")

return ascope_fzf
