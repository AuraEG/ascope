use ascope::plugin::engine::PluginEngine;
use std::fs::{self, File};
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_plugin_engine_loads_and_evaluates() {
    let dir = tempdir().unwrap();
    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir(&plugin_dir).unwrap();

    let mut toml_file = File::create(plugin_dir.join("plugin.toml")).unwrap();
    write!(
        toml_file,
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#
    )
    .unwrap();

    let mut lua_file = File::create(plugin_dir.join("init.lua")).unwrap();
    write!(
        lua_file,
        r#"
        ascope.on("key", function(key)
            return "handled: " .. key
        end)
    "#
    )
    .unwrap();

    let mut engine = PluginEngine::new(dir.path().to_path_buf()).unwrap();
    engine.load_plugins().unwrap();

    let res = engine.trigger_event("key", "Ctrl-S".to_string()).unwrap();
    assert_eq!(res, vec!["handled: Ctrl-S".to_string()]);
}

#[test]
fn test_plugin_engine_search_api() {
    let dir = tempdir().unwrap();
    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    {
        let mut toml_file = File::create(plugin_dir.join("plugin.toml")).unwrap();
        write!(
            toml_file,
            r#"
            name = "my-plugin"
            version = "0.1.0"
            author = "Developer"
            main = "init.lua"
        "#
        )
        .unwrap();
    }

    // Create a file to search
    {
        let search_target = dir.path().join("test_search.txt");
        let mut st = File::create(&search_target).unwrap();
        writeln!(st, "this is a special query string match").unwrap();
    }

    {
        let mut lua_file = File::create(plugin_dir.join("init.lua")).unwrap();
        write!(
            lua_file,
            r#"
            ascope.on("test_search", function(query)
                local results = ascope.search(query)
                if #results > 0 then
                    return "found: " .. results[1].text
                else
                    return "not found"
                end
            end)
        "#
        )
        .unwrap();
    }

    // We initialize the plugin engine with dir.path() / .config/ascope/plugins
    let config_dir = dir.path().join(".config/ascope/plugins");
    fs::create_dir_all(&config_dir).unwrap();
    // Copy the plugin to the plugins directory
    let dest_plugin = config_dir.join("my-plugin");
    fs::create_dir_all(&dest_plugin).unwrap();
    fs::copy(
        plugin_dir.join("plugin.toml"),
        dest_plugin.join("plugin.toml"),
    )
    .unwrap();
    fs::copy(plugin_dir.join("init.lua"), dest_plugin.join("init.lua")).unwrap();

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    let res = engine
        .trigger_event("test_search", "special query".to_string())
        .unwrap();
    assert_eq!(
        res,
        vec!["found: this is a special query string match\n".to_string()]
    );
}

#[test]
fn test_plugin_state_queries() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a dummy plugin that hooks an event to call state queries and return the results
    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    let mut toml_file = File::create(plugin_dir.join("plugin.toml")).unwrap();
    write!(
        toml_file,
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#
    )
    .unwrap();

    let mut lua_file = File::create(plugin_dir.join("init.lua")).unwrap();
    write!(
        lua_file,
        r#"
        ascope.on("query_state", function(payload)
            local cwd = ascope.get_cwd()
            local selection = ascope.get_selection()
            local tab_list = ascope.get_tab_list()
            local active_tab = ascope.get_active_tab()

            local sel_name = selection and selection.name or "none"
            local first_tab_path = tab_list[1] and tab_list[1].path or "none"
            local active_tab_path = active_tab and active_tab.path or "none"

            return cwd .. "|" .. sel_name .. "|" .. first_tab_path .. "|" .. active_tab_path
        end)
    "#
    )
    .unwrap();

    // Instantiate AppState
    let mut state = ascope::app::AppState::new(root.clone());

    // Copy the plugin to the expected plugins directory inside the workspace config
    let config_dir = root.join(".config/ascope/plugins");
    fs::create_dir_all(&config_dir).unwrap();
    let dest_plugin = config_dir.join("my-plugin");
    fs::create_dir_all(&dest_plugin).unwrap();
    fs::copy(
        plugin_dir.join("plugin.toml"),
        dest_plugin.join("plugin.toml"),
    )
    .unwrap();
    fs::copy(plugin_dir.join("init.lua"), dest_plugin.join("init.lua")).unwrap();

    // Load plugins on the state's plugin engine
    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    // Set the thread-local state pointer
    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);

    let res = engine.trigger_event("query_state", String::new()).unwrap();
    ascope::plugin::engine::clear_current_app_state();

    assert!(!res.is_empty());
    let parts: Vec<&str> = res[0].split('|').collect();
    assert_eq!(parts[0], root.to_string_lossy().as_ref());
    // Since my-plugin directory exists inside temp root, selection will be my-plugin
    assert_eq!(parts[1], "my-plugin");
    assert_eq!(parts[2], root.to_string_lossy().as_ref());
    assert_eq!(parts[3], root.to_string_lossy().as_ref());
}

