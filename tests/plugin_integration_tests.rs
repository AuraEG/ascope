use ascope::app::AppState;
use tempfile::tempdir;

#[test]
fn test_appstate_initializes_plugin_engine() {
    let dir = tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf());
    
    // Engine must be initialized (even if plugin folder is empty)
    assert!(state.plugin_engine.is_some());
}
