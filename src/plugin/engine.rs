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

        let search_dir_clone = plugin_dir
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.parent())
            .map(|p| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let search_fn = lua.create_function(move |lua, query: String| {
            if query.is_empty() {
                return lua.create_table();
            }

            // Run rg synchronously
            let child_res = std::process::Command::new("rg")
                .args([
                    "--json",
                    "-S", // Smart case matching
                    "--line-number",
                    "--column",
                    "--no-heading",
                    "--color=never",
                    "--glob=!node_modules",
                    "--glob=!target",
                    "--glob=!.git",
                    &query,
                    &search_dir_clone.to_string_lossy(),
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped()) // Capture stderr
                .spawn();

            let results_table = lua.create_table()?;

            if let Ok(mut child) = child_res {
                let mut stderr_capture = None;
                if let Some(stderr) = child.stderr.take() {
                    stderr_capture = Some(stderr);
                }

                if let Some(stdout) = child.stdout.take() {
                    use std::io::{BufRead, BufReader};
                    let reader = BufReader::new(stdout);
                    let mut current_file: Option<String> = None;
                    let mut idx = 1;

                    for line in reader.lines().map_while(Result::ok) {
                        if let Ok(val) = serde_json::from_str::<serde_json::Value>(&line) {
                            if let Some(msg_type) = val.get("type").and_then(|t| t.as_str()) {
                                match msg_type {
                                    "begin" => {
                                        if let Some(path_val) = val
                                            .get("data")
                                            .and_then(|d| d.get("path"))
                                            .and_then(|p| p.get("text"))
                                            .and_then(|t| t.as_str())
                                        {
                                            current_file = Some(path_val.to_string());
                                        }
                                    }
                                    "match" => {
                                        let line_number = val
                                            .get("data")
                                            .and_then(|d| d.get("line_number"))
                                            .and_then(|l| l.as_u64())
                                            .unwrap_or(0)
                                            as usize;

                                        let text = val
                                            .get("data")
                                            .and_then(|d| d.get("lines"))
                                            .and_then(|l| l.get("text"))
                                            .and_then(|t| t.as_str())
                                            .unwrap_or("")
                                            .to_string();

                                        if let Some(ref path) = current_file {
                                            let match_tbl = lua.create_table()?;
                                            match_tbl.set("path", path.clone())?;
                                            match_tbl.set("line_number", line_number)?;
                                            match_tbl.set("text", text)?;
                                            results_table.set(idx, match_tbl)?;
                                            idx += 1;

                                            if idx > 200 {
                                                break;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
                if let Some(mut stderr) = stderr_capture {
                    use std::io::Read;
                    let mut err_str = String::new();
                    let _ = stderr.read_to_string(&mut err_str);
                    if !err_str.is_empty() {
                        println!("PluginEngine rg stderr: {}", err_str);
                    }
                }
            } else if let Err(ref e) = child_res {
                println!("PluginEngine rg spawn error: {:?}", e);
            }

            Ok(results_table)
        })?;
        ascope_api.set("search", search_fn)?;

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
