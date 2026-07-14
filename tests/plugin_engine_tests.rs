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

#[test]
fn test_plugin_action_dispatch() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a target navigation path
    let target_dir = root.join("target_dir");
    fs::create_dir_all(&target_dir).unwrap();

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
        ascope.on("dispatch_actions", function(target_path)
            ascope.navigate(target_path)
            ascope.open_tab(target_path)
            ascope.notify("Hello from plugin", "info")
            return "ok"
        end)
    "#
    )
    .unwrap();

    // Instantiate AppState
    let mut state = ascope::app::AppState::new(root.clone());

    // Copy the plugin to config plugins directory
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.notification = None;
    let res = engine
        .trigger_event("dispatch_actions", target_dir.to_string_lossy().to_string())
        .unwrap();
    ascope::plugin::engine::clear_current_app_state();

    assert_eq!(res, vec!["ok".to_string()]);
    // Check navigation happened
    assert_eq!(state.current_path, target_dir);
    // Check tabs count
    assert_eq!(state.tabs.len(), 2);
    // Check notification message
    assert!(state.notification.is_some());
    let (msg, _) = state.notification.clone().unwrap();
    assert_eq!(msg, "[INFO] Hello from plugin");
}

#[test]
fn test_plugin_modal_overlay() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#,
    )
    .unwrap();

    fs::write(
        plugin_dir.join("init.lua"),
        r#"
        ascope.on("trigger_modal", function(payload)
            ascope.open_modal({
                title = "My Modal",
                items = {
                    { label = "Item 1", value = "val1" },
                    { label = "Item 2", value = "val2" }
                },
                on_select = function(item, mode)
                    ascope.notify("selected: " .. item.value .. " mode: " .. mode, "info")
                end
            })
            return "modal_opened"
        end)
    "#,
    )
    .unwrap();

    // Instantiate AppState
    let mut state = ascope::app::AppState::new(root.clone());

    // Copy the plugin to config plugins directory
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    // Bind state's plugin engine to our local engine instance so we can trigger callbacks
    state.plugin_engine = Some(engine);

    // Trigger the open modal event
    let res = state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("trigger_modal", String::new())
        .unwrap();

    assert_eq!(res, vec!["modal_opened".to_string()]);
    // Check that AppState modal mode is set correctly
    assert_eq!(state.modal_mode, ascope::app::ModalMode::PluginOverlay);
    assert_eq!(state.plugin_modal_title, "My Modal");
    assert_eq!(state.plugin_modal_items.len(), 2);
    assert_eq!(state.plugin_modal_items[0].label, "Item 1");
    assert_eq!(state.plugin_modal_items[1].value, "val2");

    state.notification = None;
    // Trigger selection callback simulation
    let engine_ref = state.plugin_engine.as_ref().unwrap();
    engine_ref
        .trigger_modal_select("val2".to_string(), "select".to_string())
        .unwrap();

    // Check that the callback triggered notification in AppState
    assert!(state.notification.is_some());
    let (msg, _) = state.notification.clone().unwrap();
    assert_eq!(msg, "[INFO] selected: val2 mode: select");

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_plugin_exec_shell() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#,
    )
    .unwrap();

    fs::write(
        plugin_dir.join("init.lua"),
        r#"
        ascope.on("trigger_exec", function(payload)
            ascope.exec_shell("echo", {"hello_world"}, function(stdout, stderr, exit_code)
                ascope.notify("output: " .. stdout .. " exit: " .. tostring(exit_code), "info")
            end)
            return "exec_called"
        end)
    "#,
    )
    .unwrap();

    // Instantiate AppState
    let mut state = ascope::app::AppState::new(root.clone());

    // Copy the plugin to config plugins directory
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.plugin_engine = Some(engine);

    state.notification = None;
    // Trigger the exec event
    let res = state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("trigger_exec", String::new())
        .unwrap();

    assert_eq!(res, vec!["exec_called".to_string()]);

    // Wait a brief moment or poll updates directly until we receive the message
    let start_time = std::time::Instant::now();
    while state.notification.is_none() && start_time.elapsed().as_secs() < 5 {
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.poll_shell_updates();
    }

    assert!(state.notification.is_some());
    let (msg, _) = state.notification.clone().unwrap();
    assert!(msg.contains("hello_world"));
    assert!(msg.contains("exit: 0"));

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_plugin_lifecycle_events() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a subfolder to navigate into
    let subfolder = root.join("subfolder");
    fs::create_dir_all(&subfolder).unwrap();

    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#,
    )
    .unwrap();

    fs::write(
        plugin_dir.join("init.lua"),
        r#"
        local events = {}
        ascope.on("on_startup", function(p)
            table.insert(events, "startup")
        end)
        ascope.on("on_enter", function(p)
            table.insert(events, "enter:" .. p)
        end)
        ascope.on("on_file_select", function(p)
            table.insert(events, "select:" .. p)
        end)
        ascope.on("on_tab_change", function(p)
            table.insert(events, "tab:" .. p)
        end)
        ascope.on("on_shutdown", function(p)
            table.insert(events, "shutdown")
        end)

        ascope.on("get_recorded_events", function()
            return table.concat(events, ",")
        end)
    "#,
    )
    .unwrap();

    // Instantiate AppState
    let mut state = ascope::app::AppState::new(root.clone());

    // Copy the plugin to config plugins directory
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.plugin_engine = Some(engine);

    // Trigger on_startup manually since we bound engine after AppState construction in test
    state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("on_startup", String::new())
        .unwrap();

    // Simulate navigation/enter directory
    state.jump_to_path(subfolder.clone());

    // Simulate selection change
    state.reset_selection_timeout();

    // Create a mock tab and change tab
    state.open_tab(root.clone());
    state.load_tab(1);

    // Simulate shutdown
    state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("on_shutdown", String::new())
        .unwrap();

    // Retrieve recorded events
    let res = state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("get_recorded_events", String::new())
        .unwrap();

    assert_eq!(res.len(), 1);
    let events_str = &res[0];
    assert!(events_str.contains("startup"));
    assert!(events_str.contains("enter:"));
    assert!(events_str.contains("tab:2"));
    assert!(events_str.contains("shutdown"));

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_plugin_keybindings_manifest() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"

        [[keybindings]]
        key = "ctrl-t"
        event = "open_tmux"

        [[keybindings]]
        key = "g z"
        event = "open_zoxide"
    "#,
    )
    .unwrap();

    fs::write(
        plugin_dir.join("init.lua"),
        r#"
        local triggered = {}
        ascope.on("open_tmux", function()
            table.insert(triggered, "tmux")
        end)
        ascope.on("open_zoxide", function()
            table.insert(triggered, "zoxide")
        end)
        ascope.on("get_triggered", function()
            return table.concat(triggered, ",")
        end)
    "#,
    )
    .unwrap();

    // Instantiate AppState
    let mut state = ascope::app::AppState::new(root.clone());

    // Copy the plugin to config plugins directory
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    // Verify keybindings are correctly parsed
    assert_eq!(engine.keybindings.len(), 2);
    assert_eq!(engine.keybindings[0].key, "ctrl-t");
    assert_eq!(engine.keybindings[0].event, "open_tmux");
    assert_eq!(engine.keybindings[1].key, "g z");
    assert_eq!(engine.keybindings[1].event, "open_zoxide");

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.plugin_engine = Some(engine);

    // Trigger keybindings directly
    state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("open_tmux", String::new())
        .unwrap();
    state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("open_zoxide", String::new())
        .unwrap();

    let res = state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("get_triggered", String::new())
        .unwrap();

    assert_eq!(res.len(), 1);
    assert_eq!(res[0], "tmux,zoxide");

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_example_tmux_bookmark_loads() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Copy the example plugin to our temporary directory to load it
    let examples_dir =
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins/tmux-bookmark");

    let config_dir = root.join(".config/ascope/plugins");
    fs::create_dir_all(&config_dir).unwrap();
    let dest_plugin = config_dir.join("tmux-bookmark");
    fs::create_dir_all(&dest_plugin).unwrap();

    fs::copy(
        examples_dir.join("plugin.toml"),
        dest_plugin.join("plugin.toml"),
    )
    .unwrap();
    fs::copy(examples_dir.join("init.lua"), dest_plugin.join("init.lua")).unwrap();

    let mut state = ascope::app::AppState::new(root.clone());
    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    // Verify keybindings parsed
    assert_eq!(engine.keybindings.len(), 1);
    assert_eq!(engine.keybindings[0].key, "ctrl-t");
    assert_eq!(engine.keybindings[0].event, "open_tmux_bookmark");

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.plugin_engine = Some(engine);

    // Verify trigger_event exists and runs without error
    let res = state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("open_tmux_bookmark", String::new());
    assert!(res.is_ok());

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_plugin_config_injection() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"

        [config]
        key_one = "value_one"
        key_two = 123
        key_three = true
    "#,
    )
    .unwrap();

    fs::write(
        plugin_dir.join("init.lua"),
        r#"
        ascope.on("trigger_config_check", function()
            local val1 = ascope.config["my-plugin"].key_one
            local val2 = ascope.config["my-plugin"].key_two
            local val3 = ascope.config["my-plugin"].key_three
            ascope.notify("one:" .. val1 .. " two:" .. tostring(val2) .. " three:" .. tostring(val3), "info")
            return "config_ok"
        end)
    "#,
    )
    .unwrap();

    let mut state = ascope::app::AppState::new(root.clone());
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.plugin_engine = Some(engine);
    state.notification = None;
    let res = state
        .plugin_engine
        .as_ref()
        .unwrap()
        .trigger_event("trigger_config_check", String::new())
        .unwrap();

    assert_eq!(res, vec!["config_ok".to_string()]);
    assert!(state.notification.is_some());
    let (msg, _) = state.notification.clone().unwrap();
    assert!(msg.contains("one:value_one"));
    assert!(msg.contains("two:123"));
    assert!(msg.contains("three:true"));

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_plugin_dynamic_keybindings() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();

    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir_all(&plugin_dir).unwrap();

    fs::write(
        plugin_dir.join("plugin.toml"),
        r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#,
    )
    .unwrap();

    fs::write(
        plugin_dir.join("init.lua"),
        r#"
        ascope.register_key("ctrl-t", function()
            ascope.notify("dynamic tmux trigger", "info")
        end)
    "#,
    )
    .unwrap();

    let mut state = ascope::app::AppState::new(root.clone());
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

    let mut engine = PluginEngine::new(config_dir).unwrap();
    engine.load_plugins().unwrap();

    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    state.plugin_engine = Some(engine);

    // Verify dynamic keybindings parsed
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()
            .len(),
        1
    );
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()[0]
            .key,
        "ctrl-t"
    );

    state.notification = None;
    // Execute the callback directly to check it works
    let engine_ref = state.plugin_engine.as_ref().unwrap();
    let key_ref = &engine_ref.dynamic_keybindings.borrow()[0].callback;
    engine_ref.execute_key_callback(key_ref).unwrap();

    assert!(state.notification.is_some());
    let (msg, _) = state.notification.clone().unwrap();
    assert_eq!(msg, "[INFO] dynamic tmux trigger");

    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_zoxide_plugin_integration() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let config_dir = root.join(".config/ascope/plugins");
    let dest_plugin = config_dir.join("zoxide");
    fs::create_dir_all(&dest_plugin).unwrap();

    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins/zoxide/plugin.toml"),
        dest_plugin.join("plugin.toml"),
    )
    .unwrap();
    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins/zoxide/init.lua"),
        dest_plugin.join("init.lua"),
    )
    .unwrap();

    let mut state = ascope::app::AppState::new(root.clone());
    let mut engine = PluginEngine::new(config_dir).unwrap();
    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    engine.load_plugins().unwrap();
    state.plugin_engine = Some(engine);

    assert_eq!(state.plugin_engine.as_ref().unwrap().keybindings.len(), 0);
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()
            .len(),
        1
    );
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()[0]
            .key,
        "shift-z"
    );
    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_fzf_plugin_integration() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let config_dir = root.join(".config/ascope/plugins");
    let dest_plugin = config_dir.join("fzf");
    fs::create_dir_all(&dest_plugin).unwrap();

    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins/fzf/plugin.toml"),
        dest_plugin.join("plugin.toml"),
    )
    .unwrap();
    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins/fzf/init.lua"),
        dest_plugin.join("init.lua"),
    )
    .unwrap();

    let mut state = ascope::app::AppState::new(root.clone());
    let mut engine = PluginEngine::new(config_dir).unwrap();
    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    engine.load_plugins().unwrap();
    state.plugin_engine = Some(engine);

    assert_eq!(state.plugin_engine.as_ref().unwrap().keybindings.len(), 0);
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()
            .len(),
        1
    );
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()[0]
            .key,
        "shift-f"
    );
    ascope::plugin::engine::clear_current_app_state();
}

#[test]
fn test_ssh_plugin_integration() {
    let dir = tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let config_dir = root.join(".config/ascope/plugins");
    let dest_plugin = config_dir.join("ssh");
    fs::create_dir_all(&dest_plugin).unwrap();

    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("examples/plugins/ssh/plugin.toml"),
        dest_plugin.join("plugin.toml"),
    )
    .unwrap();
    fs::copy(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("examples/plugins/ssh/init.lua"),
        dest_plugin.join("init.lua"),
    )
    .unwrap();

    let mut state = ascope::app::AppState::new(root.clone());
    let mut engine = PluginEngine::new(config_dir).unwrap();
    ascope::plugin::engine::set_current_app_state(&mut state as *mut ascope::app::AppState);
    engine.load_plugins().unwrap();
    state.plugin_engine = Some(engine);

    assert_eq!(state.plugin_engine.as_ref().unwrap().keybindings.len(), 0);
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()
            .len(),
        1
    );
    assert_eq!(
        state
            .plugin_engine
            .as_ref()
            .unwrap()
            .dynamic_keybindings
            .borrow()[0]
            .key,
        "alt-s"
    );
    ascope::plugin::engine::clear_current_app_state();
}
