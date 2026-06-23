use ascope::app::{AppState, ModalMode};
use tempfile::tempdir;

#[test]
fn test_search_overlay_modal_mode() {
    let dir = tempdir().unwrap();
    let mut state = AppState::new(dir.path().to_path_buf());

    // By default, modal mode should be None
    assert_eq!(state.modal_mode, ModalMode::None);

    // Set to SearchOverlay
    state.modal_mode = ModalMode::SearchOverlay;
    assert_eq!(state.modal_mode, ModalMode::SearchOverlay);
}

#[test]
fn test_search_overlay_fuzzy_matching() {
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("main.rs");
    let file2 = dir.path().join("lib.rs");
    std::fs::File::create(&file1).unwrap();
    std::fs::File::create(&file2).unwrap();

    let mut state = AppState::new(dir.path().to_path_buf());
    // Wait for scanning to complete
    for _ in 0..100 {
        state.poll_scan();
        if !state.is_scanning() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    // Set to FuzzyFiles search overlay mode
    state.modal_mode = ModalMode::SearchOverlay;
    state.search_overlay_mode = ascope::app::SearchOverlayMode::FuzzyFiles;
    state.search_overlay_input = "main".to_string();

    state.update_search_overlay_results();

    // search_overlay_results should contain main.rs
    assert!(!state.search_overlay_results.is_empty());
    assert!(state.search_overlay_results[0].text.contains("main.rs"));
}

