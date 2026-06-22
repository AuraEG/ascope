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

