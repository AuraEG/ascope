use ascope::{app::AppState, app::ModalMode};
use std::fs::File;
use tempfile::tempdir;

#[test]
fn test_lazy_loading_and_popup_transitions() {
    let temp_dir = tempdir().unwrap();
    std::fs::create_dir(temp_dir.path().join("subdir1")).unwrap();
    File::create(temp_dir.path().join("subdir1/file1.txt")).unwrap();

    let mut state = AppState::new(temp_dir.path().to_path_buf());

    // Wait for the scan of immediate children to finish
    let start = std::time::Instant::now();
    loop {
        state.poll_scan();
        if state
            .all_entries
            .iter()
            .any(|e| e.path.file_name().unwrap() == "subdir1")
        {
            break;
        }
        assert!(start.elapsed().as_secs() < 5, "Scan timed out");
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    // On startup, subdir1 should not be expanded, so recursive files should not be in all_entries
    assert!(
        !state
            .all_entries
            .iter()
            .any(|e| e.path.file_name().unwrap() == "file1.txt"),
        "file1.txt should not be loaded on startup (lazy loading)"
    );

    // Select the directory and expand it
    let subdir_idx = state
        .all_entries
        .iter()
        .position(|e| e.path.file_name().unwrap() == "subdir1")
        .expect("subdir1 not found in entries");

    // Move cursor to subdir1 (it's the first child)
    while state.navigation.cursor() != subdir_idx {
        state.move_selection(1);
    }

    // Toggle expand
    state.toggle_expand();

    // Now file1.txt should be in all_entries
    assert!(
        state
            .all_entries
            .iter()
            .any(|e| e.path.file_name().unwrap() == "file1.txt"),
        "file1.txt should be loaded after expanding subdir1"
    );

    // Let's trigger the size details popup
    state.trigger_size_details_popup();

    assert_eq!(state.modal_mode, ModalMode::SizeDetails);
    assert_eq!(state.size_popup_path, Some(temp_dir.path().join("subdir1")));
    assert!(state.size_popup_stats.is_some());
    assert!(state.size_popup_progress.is_some());

    // Close details popup
    state.close_size_details_popup();
    assert_eq!(state.modal_mode, ModalMode::None);
    assert!(state.size_popup_path.is_none());
}

#[test]
fn test_folder_dashboard_summary_calculation() {
    let temp_dir = tempdir().unwrap();
    File::create(temp_dir.path().join("file1.rs")).unwrap();
    File::create(temp_dir.path().join("file2.json")).unwrap();
    std::fs::create_dir(temp_dir.path().join("subdir")).unwrap();

    let state = AppState::new(temp_dir.path().to_path_buf());

    let summary = state.get_folder_dashboard(temp_dir.path());
    assert_eq!(summary.file_count, 2);
    assert_eq!(summary.dir_count, 1);
    assert!(summary
        .extension_counts
        .iter()
        .any(|(ext, count)| ext == "rs" && *count == 1));
    assert!(summary
        .extension_counts
        .iter()
        .any(|(ext, count)| ext == "json" && *count == 1));
}
