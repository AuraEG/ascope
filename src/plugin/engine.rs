use crate::plugin::manifest::PluginManifest;
use mlua::{Function, Lua, Table, Value};
use std::fs;
use std::path::PathBuf;

pub struct PluginEngine {
    lua: Lua,
    plugin_dir: PathBuf,
}

impl PluginEngine {
    pub fn new(plugin_dir: PathBuf) -> Result<Self, mlua::Error> {
        let lua = Lua::new();

        // Inject global api table
        let ascope_api = lua.create_table()?;
        lua.globals().set("ascope", ascope_api)?;

        Ok(Self { lua, plugin_dir })
    }

    pub fn load_plugins(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if !self.plugin_dir.exists() {
            return Ok(());
        }

        // Expose ascope.on registration helper
        let lua_instance = &self.lua;
        let globals = lua_instance.globals();
        let ascope: Table = globals.get("ascope")?;

        // Register standard hook function in Lua
        // Let's store callbacks under a private global Lua table.
        let callbacks_table = lua_instance.create_table()?;
        lua_instance.globals().set("_callbacks", callbacks_table)?;

        let on_fn = lua_instance.create_function(|lua, (event, cb): (String, Function)| {
            let callbacks: Table = lua.globals().get("_callbacks")?;
            let list: Table = match callbacks.get(event.clone()) {
                Ok(t) => t,
                _ => {
                    let t = lua.create_table()?;
                    callbacks.set(event.clone(), t.clone())?;
                    t
                }
            };
            let len = list.len()?;
            list.set(len + 1, cb)?;
            Ok(())
        })?;
        ascope.set("on", on_fn)?;

        for entry in fs::read_dir(&self.plugin_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let manifest_path = path.join("plugin.toml");
                if manifest_path.exists() {
                    let manifest_content = fs::read_to_string(&manifest_path)?;
                    let manifest: PluginManifest = toml::from_str(&manifest_content)?;
                    let script_path = path.join(&manifest.main);
                    if script_path.exists() {
                        let script_content = fs::read_to_string(&script_path)?;
                        self.lua.load(&script_content).exec()?;
                    }
                }
            }
        }
        Ok(())
    }

    pub fn trigger_event(&self, event: &str, payload: String) -> Result<Vec<String>, mlua::Error> {
        let callbacks: Table = match self.lua.globals().get::<_, Table>("_callbacks") {
            Ok(t) => t,
            Err(_) => return Ok(Vec::new()),
        };
        let mut results = Vec::new();
        if let Ok(list) = callbacks.get::<_, Table>(event) {
            let len = list.len()?;
            for i in 1..=len {
                let func: Function = list.get(i)?;
                if let Ok(Value::String(res)) = func.call::<_, Value>(payload.clone()) {
                    results.push(res.to_str()?.to_string());
                }
            }
        }
        Ok(results)
    }
}
