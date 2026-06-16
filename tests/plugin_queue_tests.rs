use ascope::app::AppState;
use ascope::plugin::commands::PluginCommand;
use tempfile::tempdir;

#[test]
fn test_command_execution() {
    let dir = tempdir().unwrap();
    let mut state = AppState::new(dir.path().to_path_buf());

    let cmd = PluginCommand::FocusPath {
        path: "/tmp".to_string(),
    };
    state.execute_plugin_command(cmd);

    assert_eq!(state.current_path.to_string_lossy(), "/tmp");
}
