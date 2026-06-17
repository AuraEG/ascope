use ascope::app::{AppState, PreviewType};
use tempfile::tempdir;

#[test]
fn test_preview_type_detection() {
    let dir = tempdir().unwrap();
    let state = AppState::new(dir.path().to_path_buf());
    
    assert_eq!(state.detect_preview_type(&std::path::PathBuf::from("image.png")), PreviewType::Image);
    assert_eq!(state.detect_preview_type(&std::path::PathBuf::from("image.JPG")), PreviewType::Image);
    assert_eq!(state.detect_preview_type(&std::path::PathBuf::from("code.rs")), PreviewType::Text);
    assert_eq!(state.detect_preview_type(&std::path::PathBuf::from("document.pdf")), PreviewType::Unsupported);
}
