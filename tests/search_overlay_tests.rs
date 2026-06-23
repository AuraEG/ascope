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
