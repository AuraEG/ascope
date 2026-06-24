use ascope::app::AppState;
use tempfile::tempdir;

#[test]
fn test_command_palette_fuzzy_matching() {
    let dir = tempdir().unwrap();
    let mut state = AppState::new(dir.path().to_path_buf());
    
    state.command_palette_candidates = vec![
        ascope::project::detector::DetectedCommand {
            name: "npm run dev".to_string(),
            cmd: "npm run dev".to_string(),
            source: "package.json".to_string(),
        },
        ascope::project::detector::DetectedCommand {
            name: "cargo test".to_string(),
            cmd: "cargo test".to_string(),
            source: "Cargo".to_string(),
        },
    ];
    state.command_palette_input = "npm".to_string();
    state.update_command_palette_results();
    
    assert_eq!(state.command_palette_results.len(), 1);
    assert_eq!(state.command_palette_results[0].name, "npm run dev");
}
