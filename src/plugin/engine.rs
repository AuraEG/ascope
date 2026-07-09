use crate::app::AppState;
use crate::plugin::manifest::PluginManifest;
use mlua::{Function, Lua, Table, Value};
use std::cell::Cell;
use std::fs;
use std::path::PathBuf;

thread_local! {
    static CURRENT_STATE: Cell<Option<*mut AppState>> = const { Cell::new(None) };
    static CURRENT_ENGINE: Cell<Option<*mut PluginEngine>> = const { Cell::new(None) };
}

pub fn set_current_engine(engine: *mut PluginEngine) {
    CURRENT_ENGINE.with(|cell| {
        cell.set(Some(engine));
    });
}

pub fn clear_current_engine() {
    CURRENT_ENGINE.with(|cell| {
        cell.set(None);
    });
}

pub fn with_current_engine_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut PluginEngine) -> R,
{
    CURRENT_ENGINE.with(|cell| {
        let ptr = cell.get()?;
        unsafe { Some(f(&mut *ptr)) }
    })
}

pub fn set_current_app_state(state: *mut AppState) {
    CURRENT_STATE.with(|cell| {
        cell.set(Some(state));
    });
}

pub fn clear_current_app_state() {
    CURRENT_STATE.with(|cell| {
        cell.set(None);
    });
}

pub fn with_app_state<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&AppState) -> R,
{
    CURRENT_STATE.with(|cell| {
        let ptr = cell.get()?;
        unsafe { Some(f(&*ptr)) }
    })
}

pub fn with_app_state_mut<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut AppState) -> R,
{
    CURRENT_STATE.with(|cell| {
        let ptr = cell.get()?;
        unsafe { Some(f(&mut *ptr)) }
    })
}

#[derive(Debug)]
pub struct DynamicKeybinding {
    pub key: String,
    pub callback: mlua::RegistryKey,
    pub description: Option<String>,
}

pub struct PluginEngine {
    lua: Lua,
    plugin_dir: PathBuf,
    pub active_modal_callback: std::cell::RefCell<Option<mlua::RegistryKey>>,
    pub keybindings: Vec<crate::plugin::manifest::Keybinding>,
    pub dynamic_keybindings: std::cell::RefCell<Vec<DynamicKeybinding>>,
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

        let get_cwd_fn = lua.create_function(|_, ()| {
            let cwd = with_app_state(|state| state.current_path.to_string_lossy().to_string())
                .unwrap_or_default();
            Ok(cwd)
        })?;
        ascope_api.set("get_cwd", get_cwd_fn)?;

        let get_selection_fn = lua.create_function(|lua, ()| {
            let selection = with_app_state(|state| {
                if let Some(entry) = state.navigation.current_selection() {
                    if let Ok(tbl) = lua.create_table() {
                        let _ = tbl.set("path", entry.path.to_string_lossy().to_string());
                        let _ = tbl.set(
                            "name",
                            entry
                                .path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        );
                        let _ = tbl.set(
                            "is_dir",
                            matches!(entry.entry_type, crate::fs::walker::EntryType::Directory),
                        );
                        let _ = tbl.set("size", entry.size);
                        return Some(tbl);
                    }
                }
                None
            })
            .flatten();
            Ok(selection)
        })?;
        ascope_api.set("get_selection", get_selection_fn)?;

        let get_tab_list_fn = lua.create_function(|lua, ()| {
            let tabs_table = lua.create_table()?;
            with_app_state(|state| {
                for (idx, tab) in state.tabs.iter().enumerate() {
                    if let Ok(tab_tbl) = lua.create_table() {
                        let _ = tab_tbl.set("id", idx + 1);
                        let _ = tab_tbl.set("path", tab.current_path.to_string_lossy().to_string());
                        let _ = tab_tbl.set(
                            "label",
                            tab.current_path
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .to_string(),
                        );
                        let _ = tabs_table.set(idx + 1, tab_tbl);
                    }
                }
            });
            Ok(tabs_table)
        })?;
        ascope_api.set("get_tab_list", get_tab_list_fn)?;

        let get_active_tab_fn = lua.create_function(|lua, ()| {
            let active_tab = with_app_state(|state| {
                let idx = state.active_tab;
                if let Some(tab) = state.tabs.get(idx) {
                    if let Ok(tbl) = lua.create_table() {
                        let _ = tbl.set("id", idx + 1);
                        let _ = tbl.set("path", tab.current_path.to_string_lossy().to_string());
                        return Some(tbl);
                    }
                }
                None
            })
            .flatten();
            Ok(active_tab)
        })?;
        ascope_api.set("get_active_tab", get_active_tab_fn)?;

        let navigate_fn = lua.create_function(|_, path: String| {
            let path_buf = PathBuf::from(path);
            with_app_state_mut(|state| {
                let full_path = if path_buf.is_absolute() {
                    path_buf.clone()
                } else {
                    state.current_path.join(&path_buf)
                };

                if full_path.is_file() {
                    if let Some(parent) = full_path.parent() {
                        state.jump_to_path(parent.to_path_buf());
                        let file_name = full_path.file_name().unwrap_or_default();
                        if let Some(idx) = state
                            .all_entries
                            .iter()
                            .position(|entry| entry.path.file_name() == Some(file_name))
                        {
                            state.navigation.set_cursor(idx);
                        }
                    }
                } else {
                    state.jump_to_path(path_buf);
                }
            });
            Ok(())
        })?;
        ascope_api.set("navigate", navigate_fn)?;

        let open_tab_fn = lua.create_function(|_, path: String| {
            let path_buf = PathBuf::from(path);
            with_app_state_mut(|state| {
                state.open_tab(path_buf);
            });
            Ok(())
        })?;
        ascope_api.set("open_tab", open_tab_fn)?;

        let close_tab_fn = lua.create_function(|_, id: Option<usize>| {
            with_app_state_mut(|state| {
                if let Some(id_val) = id {
                    if id_val > 0 {
                        state.close_tab_at(id_val - 1);
                    }
                } else {
                    state.close_tab();
                }
            });
            Ok(())
        })?;
        ascope_api.set("close_tab", close_tab_fn)?;

        let notify_fn = lua.create_function(|_, (msg, level): (String, Option<String>)| {
            let formatted = if let Some(lvl) = level {
                format!("[{}] {}", lvl.to_uppercase(), msg)
            } else {
                msg
            };
            with_app_state_mut(|state| {
                state.notification = Some((formatted, std::time::Instant::now()));
            });
            Ok(())
        })?;
        ascope_api.set("notify", notify_fn)?;

        let open_modal_fn = lua.create_function(|lua, opts: Table| {
            let title: String = opts.get("title").unwrap_or_else(|_| "Overlay".to_string());
            let items_table: Table = opts.get("items")?;
            let mut items = Vec::new();
            let len = items_table.len()?;
            for i in 1..=len {
                let item_tbl: Table = items_table.get(i)?;
                let label: String = item_tbl.get("label")?;
                let value: String = item_tbl.get("value")?;
                items.push(crate::app::PluginOverlayItem { label, value });
            }

            let on_select: Function = opts.get("on_select")?;
            let key = lua.create_registry_value(on_select)?;

            with_app_state_mut(|state| {
                state.modal_mode = crate::app::ModalMode::PluginOverlay;
                state.plugin_modal_title = title;
                state.plugin_modal_items = items.clone();
                state.plugin_modal_filtered_items = items;
                state.plugin_modal_input.clear();
                state.plugin_modal_selected_index = 0;
                state.plugin_modal_cursor_index = 0;
                state.plugin_modal_focused = true;
                state.update_plugin_modal_preview();
            });

            with_app_state_mut(|state| {
                if let Some(ref mut engine) = state.plugin_engine {
                    *engine.active_modal_callback.borrow_mut() = Some(key);
                }
            });

            Ok(())
        })?;
        ascope_api.set("open_modal", open_modal_fn)?;

        let close_modal_fn = lua.create_function(|_, ()| {
            with_app_state_mut(|state| {
                state.modal_mode = crate::app::ModalMode::None;
                if let Some(ref mut engine) = state.plugin_engine {
                    *engine.active_modal_callback.borrow_mut() = None;
                }
            });
            Ok(())
        })?;
        ascope_api.set("close_modal", close_modal_fn)?;

        let open_in_editor_fn = lua.create_function(|_, path: String| {
            let path_buf = PathBuf::from(path);
            let full_path = with_app_state(|state| {
                if path_buf.is_absolute() {
                    path_buf.clone()
                } else {
                    state.current_path.join(&path_buf)
                }
            })
            .unwrap_or(path_buf);

            use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
            use crossterm::execute;
            use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
            use crossterm::terminal::{EnterAlternateScreen, LeaveAlternateScreen};

            let _ = disable_raw_mode();
            let mut stdout = std::io::stdout();
            let _ = execute!(
                stdout,
                LeaveAlternateScreen,
                DisableMouseCapture,
                crossterm::cursor::Show
            );

            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "nvim".to_string());
            let mut cmd = std::process::Command::new(&editor);
            cmd.arg(&full_path);

            if let Ok(mut child) = cmd.spawn() {
                let _ = child.wait();
            }

            let _ = enable_raw_mode();
            let _ = execute!(
                stdout,
                EnterAlternateScreen,
                EnableMouseCapture,
                crossterm::cursor::Hide
            );

            with_app_state_mut(|state| {
                state.needs_terminal_clear = true;
            });

            Ok(())
        })?;
        ascope_api.set("open_in_editor", open_in_editor_fn)?;

        let open_in_default_app_fn = lua.create_function(|_, path: String| {
            let path_buf = PathBuf::from(path);
            let full_path = with_app_state(|state| {
                if path_buf.is_absolute() {
                    path_buf.clone()
                } else {
                    state.current_path.join(&path_buf)
                }
            })
            .unwrap_or(path_buf);

            let _ = std::process::Command::new("xdg-open")
                .arg(full_path)
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
            Ok(())
        })?;
        ascope_api.set("open_in_default_app", open_in_default_app_fn)?;

        let exec_shell_fn =
            lua.create_function(|lua, (cmd, args, cb): (String, Table, Function)| {
                let mut args_vec = Vec::new();
                let len = args.len()?;
                for i in 1..=len {
                    let val: String = args.get(i)?;
                    args_vec.push(val);
                }

                let key = lua.create_registry_value(cb)?;

                let tx_opt = with_app_state(|state| state.shell_result_tx.clone());

                if let Some(tx) = tx_opt {
                    std::thread::spawn(move || {
                        let mut command = std::process::Command::new(&cmd);
                        command.args(&args_vec);
                        command.stdout(std::process::Stdio::piped());
                        command.stderr(std::process::Stdio::piped());

                        let output_res = command.output();
                        let (stdout, stderr, exit_code) = match output_res {
                            Ok(output) => {
                                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                                let exit_code = output.status.code();
                                (stdout, stderr, exit_code)
                            }
                            Err(e) => (
                                String::new(),
                                format!("Failed to execute command: {e}"),
                                None,
                            ),
                        };

                        let _ = tx.send(crate::app::ShellResult {
                            callback_key: key,
                            stdout,
                            stderr,
                            exit_code,
                        });
                    });
                }

                Ok(())
            })?;
        ascope_api.set("exec_shell", exec_shell_fn)?;

        let register_key_fn = lua.create_function(
            |lua, (binding, cb, description): (String, Function, Option<String>)| {
                let key = lua.create_registry_value(cb)?;
                with_current_engine_mut(|engine| {
                    engine
                        .dynamic_keybindings
                        .borrow_mut()
                        .push(DynamicKeybinding {
                            key: binding,
                            callback: key,
                            description,
                        });
                });
                Ok(())
            },
        )?;
        ascope_api.set("register_key", register_key_fn)?;

        let config_tbl = lua.create_table()?;
        ascope_api.set("config", config_tbl)?;

        lua.globals().set("ascope", ascope_api)?;

        Ok(Self {
            lua,
            plugin_dir,
            active_modal_callback: std::cell::RefCell::new(None),
            keybindings: Vec::new(),
            dynamic_keybindings: std::cell::RefCell::new(Vec::new()),
        })
    }

    pub fn load_plugins(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        set_current_engine(self as *mut Self);
        let res = self.load_plugins_inner();
        clear_current_engine();
        res
    }

    fn load_plugins_inner(&mut self) -> Result<(), Box<dyn std::error::Error>> {
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
                    self.keybindings.extend(manifest.keybindings.clone());

                    let config_val = toml_to_lua(&self.lua, &toml::Value::Table(manifest.config))?;
                    let ascope: Table = self.lua.globals().get("ascope")?;
                    let configs_table: Table = ascope.get("config")?;
                    configs_table.set(manifest.name.as_str(), config_val)?;
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

    pub fn trigger_modal_select(&self, value: String, mode: String) -> Result<(), mlua::Error> {
        let key_opt = self.active_modal_callback.borrow_mut().take();
        if let Some(key) = key_opt {
            let func: Function = self.lua.registry_value(&key)?;
            let tbl = self.lua.create_table()?;
            tbl.set("value", value)?;
            let _res: Value = func.call::<_, Value>((tbl, mode))?;
            self.lua.remove_registry_value(key)?;
        }
        Ok(())
    }

    pub fn clear_modal_callback(&self) -> Result<(), mlua::Error> {
        let key_opt = self.active_modal_callback.borrow_mut().take();
        if let Some(key) = key_opt {
            self.lua.remove_registry_value(key)?;
        }
        Ok(())
    }

    pub fn execute_shell_callback(
        &self,
        key: mlua::RegistryKey,
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
    ) -> Result<(), mlua::Error> {
        let func: Function = self.lua.registry_value(&key)?;
        let _res: Value = func.call::<_, Value>((stdout, stderr, exit_code))?;
        self.lua.remove_registry_value(key)?;
        Ok(())
    }

    pub fn execute_key_callback(&self, key: &mlua::RegistryKey) -> Result<(), mlua::Error> {
        let func: Function = self.lua.registry_value(key)?;
        let _res: Value = func.call::<_, Value>(())?;
        Ok(())
    }
}

fn toml_to_lua<'lua>(lua: &'lua Lua, val: &toml::Value) -> Result<Value<'lua>, mlua::Error> {
    match val {
        toml::Value::String(s) => Ok(Value::String(lua.create_string(s)?)),
        toml::Value::Integer(i) => Ok(Value::Integer(*i)),
        toml::Value::Float(f) => Ok(Value::Number(*f)),
        toml::Value::Boolean(b) => Ok(Value::Boolean(*b)),
        toml::Value::Datetime(d) => Ok(Value::String(lua.create_string(d.to_string())?)),
        toml::Value::Array(arr) => {
            let table = lua.create_table()?;
            for (i, v) in arr.iter().enumerate() {
                table.set(i + 1, toml_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
        toml::Value::Table(tbl) => {
            let table = lua.create_table()?;
            for (k, v) in tbl.iter() {
                table.set(k.as_str(), toml_to_lua(lua, v)?)?;
            }
            Ok(Value::Table(table))
        }
    }
}
