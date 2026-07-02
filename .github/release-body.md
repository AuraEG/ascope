AuraScope v0.8.0 "Integration Plugins Foundation" introduces a powerful, sandbox-safe Lua plugin system. Plugins can interact dynamically with AuraScope's state, trigger TUI actions, execute background shell commands, register custom hotkeys, listen to lifecycle event triggers, and parse configuration blocks.

### Summary of Changes

* **State Query API**: Exposes thread-safe Lua methods (`ascope.get_cwd`, `ascope.get_selection`, `ascope.get_tab_list`, `ascope.get_active_tab`) to query active workspaces and tab details.
* **Action Dispatch API**: Exposes core navigation triggers (`ascope.navigate`, `ascope.open_tab`, `ascope.close_tab`, `ascope.notify`) to programmatically manage directories and notifications.
* **Interactive Modal/Overlay Picker**: Provides a cyan-themed custom list-picker overlay (`ascope.open_modal`) with backspace and traversal support.
* **Async Shell Executor**: Provides a non-blocking subprocess command runner (`ascope.exec_shell`) using channels to return exit codes and outputs back to the main thread.
* **Dynamic Keybindings**: Registers hotkey combinations (`ascope.register_key`) supporting multi-key sequence prefix matching and event interception.
* **Plugin Config Injection**: Serializes TOML manifest `[config]` sections directly into Lua (`ascope.config[plugin_name]`).
* **Lifecycle Event System**: Fires hooks for key application moments: `on_startup`, `on_enter`, `on_file_select`, `on_tab_change`, and `on_shutdown`.
* **Example Plugin**: Includes a fully functional tmux session manager plugin under `examples/plugins/tmux-bookmark`.

### Commit History and Roles

* `d8008d9` - `Merge branch 'feat/task-0.7-documentation-and-examples' into develop`
  * **Role**: Merge config injection, dynamic keybindings, documentation, and example plugins.
* `f8b640c` - `Merge branch 'feat/task-0.6-event-driven-keybinding-trigger-maps' into develop`
  * **Role**: Merge manifest-driven event keybindings.
* `e459610` - `Merge branch 'feat/task-0.5-lifecycle-events-expansion' into develop`
  * **Role**: Merge app entry/exit lifecycle event hooks.
* `1bf5a01` - `Merge branch 'feat/task-0.4-shell-execution-api' into develop`
  * **Role**: Merge safe background command runner.
* `0eaa1a5` - `Merge branch 'feat/task-0.3-modal-overlay-api' into develop`
  * **Role**: Merge interactive cyan selection overlay picker.
* `a6c524c` - `Merge branch 'feat/task-0.2-action-dispatch-api' into develop`
  * **Role**: Merge directory navigation and notification actions.
* `240239a` - `feat(plugin): implement Task 0.2 Action Dispatch API`
  * **Role**: Implement TUI action bindings in engine.
* `2b999f6` - `feat(plugin): implement Task 0.4 Shell Execution API (safe, async)`
  * **Role**: Implement channel-based process spawning.
