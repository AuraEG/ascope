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

#[test]
fn test_search_overlay_live_grep() {
    use std::io::Write;
    let dir = tempdir().unwrap();
    let file1 = dir.path().join("test.txt");
    let mut f1 = std::fs::File::create(&file1).unwrap();
    f1.write_all(b"rust is extremely fast\n").unwrap();

    let mut state = AppState::new(dir.path().to_path_buf());
    // Wait for scan to finish
    for _ in 0..100 {
        state.poll_scan();
        if !state.is_scanning() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    state.modal_mode = ModalMode::SearchOverlay;
    state.search_overlay_mode = ascope::app::SearchOverlayMode::LiveGrep;
    state.search_overlay_input = "extremely".to_string();

    state.update_search_overlay_results();

    // Poll channel for matches
    let start = std::time::Instant::now();
    loop {
        state.poll_search_updates();
        if !state.search_overlay_results.is_empty() {
            break;
        }
        if start.elapsed().as_secs() > 5 {
            panic!("Live grep did not return any matches in time");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    assert_eq!(state.search_overlay_results.len(), 1);
    assert!(state.search_overlay_results[0].text.contains("rust is extremely fast"));
}

