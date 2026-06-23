use ascope::app::{AppState, PreviewType};
use tempfile::tempdir;

#[test]
fn test_preview_type_detection() {
    let dir = tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf());

    assert_eq!(
        state.detect_preview_type(&std::path::PathBuf::from("image.png")),
        PreviewType::Image
    );
    assert_eq!(
        state.detect_preview_type(&std::path::PathBuf::from("image.JPG")),
        PreviewType::Image
    );
    assert_eq!(
        state.detect_preview_type(&std::path::PathBuf::from("code.rs")),
        PreviewType::Text
    );
    assert_eq!(
        state.detect_preview_type(&std::path::PathBuf::from("document.pdf")),
        PreviewType::Image
    );
}

#[test]
fn test_selection_debounce_timer() {
    let dir = tempdir().unwrap();
    std::fs::File::create(dir.path().join("file.txt")).unwrap();
    let mut state = AppState::new(dir.path().to_path_buf());

    // Wait for scanning to complete
    for _ in 0..100 {
        state.poll_scan();
        if !state.is_scanning() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    let t0 = state.last_selection_time;

    std::thread::sleep(std::time::Duration::from_millis(5));
    state.move_selection(1);

    assert!(state.last_selection_time > t0);
}

#[test]
fn test_preview_search_overlay_highlight() {
    use ascope::app::{ModalMode, SearchMatch, SearchOverlayMode};
    use std::fs::File;
    use std::io::Write;

    let dir = tempdir().unwrap();
    let file_path = dir.path().join("code.rs");
    let mut f = File::create(&file_path).unwrap();
    f.write_all(b"line 1\nline 2\nline 3\nline 4\nline 5\n")
        .unwrap();

    let mut state = AppState::new(dir.path().to_path_buf());

    // Wait for scanning to complete
    for _ in 0..100 {
        state.poll_scan();
        if !state.is_scanning() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }

    state.modal_mode = ModalMode::SearchOverlay;
    state.search_overlay_mode = SearchOverlayMode::LiveGrep;
    state.search_overlay_input = "line 3".to_string();
    state.search_overlay_results = vec![SearchMatch {
        path: file_path.clone(),
        line_number: Some(3),
        text: "line 3".to_string(),
    }];
    state.search_overlay_selected_index = 0;

    assert!(state.preview_lines().is_empty());

    state.update_preview_cache(80, 24);

    let lines = state.preview_lines();
    assert!(!lines.is_empty());

    let preview_text: String = lines
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        preview_text.contains("line 3"),
        "Preview should contain the matched line content"
    );

    // Now change the selected search match to another line (line 5)
    state.search_overlay_results = vec![SearchMatch {
        path: file_path.clone(),
        line_number: Some(5),
        text: "line 5".to_string(),
    }];
    state.update_preview_cache(80, 24);
    let new_preview = state.preview_lines();
    let new_preview_text: String = new_preview
        .iter()
        .map(|l| l.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        new_preview_text.contains("line 5"),
        "Preview should update to center on the new line"
    );
}
