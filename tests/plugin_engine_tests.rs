use ascope::plugin::engine::PluginEngine;
use tempfile::tempdir;
use std::fs::{self, File};
use std::io::Write;

#[test]
fn test_plugin_engine_loads_and_evaluates() {
    let dir = tempdir().unwrap();
    let plugin_dir = dir.path().join("my-plugin");
    fs::create_dir(&plugin_dir).unwrap();

    let mut toml_file = File::create(plugin_dir.join("plugin.toml")).unwrap();
    write!(toml_file, r#"
        name = "my-plugin"
        version = "0.1.0"
        author = "Developer"
        main = "init.lua"
    "#).unwrap();

    let mut lua_file = File::create(plugin_dir.join("init.lua")).unwrap();
    write!(lua_file, r#"
        ascope.on("key", function(key)
            return "handled: " .. key
        end)
    "#).unwrap();

    let mut engine = PluginEngine::new(dir.path().to_path_buf()).unwrap();
    engine.load_plugins().unwrap();

    let res = engine.trigger_event("key", "Ctrl-S".to_string()).unwrap();
    assert_eq!(res, vec!["handled: Ctrl-S".to_string()]);
}
