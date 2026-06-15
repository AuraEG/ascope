use ascope::{app::AppState, navigation::Direction};
use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_navigation_in_real_appstate() {
    let temp_dir = tempdir().unwrap();
    File::create(temp_dir.path().join("file1.txt")).unwrap();
    File::create(temp_dir.path().join("file2.txt")).unwrap();
    std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

    let mut state = AppState::new(temp_dir.path().to_path_buf());

    // Wait for scan to complete
    while state.is_scanning() {
        state.poll_scan();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    state.poll_scan();

    // Test cursor movement
    let visible_count = state.navigation.visible_items().len();
    assert!(visible_count >= 3, "Should have at least 3 entries");

    state.navigation.move_cursor(Direction::Down);
    let selected = state.navigation.current_selection();
    assert!(selected.is_some(), "Should have selection after moving");
}

#[test]
fn test_filtering_in_appstate() {
    let temp_dir = tempdir().unwrap();
    File::create(temp_dir.path().join("src_file.txt")).unwrap();
    File::create(temp_dir.path().join("test_file.txt")).unwrap();

    let mut state = AppState::new(temp_dir.path().to_path_buf());

    while state.is_scanning() {
        state.poll_scan();
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    state.poll_scan();

    // Apply filter
    state
        .navigation
        .set_filter(Some("src".to_string()), &state.all_entries);
    let visible = state.visible_items();
    assert_eq!(visible.len(), 1);
    assert!(visible[0].0.path.to_string_lossy().contains("src"));
}
