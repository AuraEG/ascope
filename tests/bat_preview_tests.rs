use ascope::ui::widgets::{build_preview_lines, is_using_bat_previewer};
use std::fs::File;
use std::io::Write;
use tempfile::tempdir;

#[test]
fn test_is_using_bat_previewer() {
    assert!(is_using_bat_previewer());
}

#[test]
fn test_bat_highlights_rust_code() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("main.rs");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"fn main() {\n    println!(\"hello\");\n}\n").unwrap();

    let lines = build_preview_lines(&file_path, "");
    assert!(!lines.is_empty());
    
    // The preview should contain "fn main()" text
    let text = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("fn main()"));
}

#[test]
fn test_bat_highlights_custom_extensions() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("index.tsx");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"const App = () => <div>Hello</div>;\nexport default App;\n").unwrap();

    let lines = build_preview_lines(&file_path, "");
    assert!(!lines.is_empty());
    let text = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("const App"));
}

#[test]
fn test_bat_highlights_markdown() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test.md");
    let mut file = File::create(&file_path).unwrap();
    file.write_all(b"# Header\n**bold text**\nnormal text\n").unwrap();

    let lines = build_preview_lines(&file_path, "");
    assert!(!lines.is_empty());
    let text = lines.iter().map(|l| l.to_string()).collect::<Vec<_>>().join("\n");
    assert!(text.contains("Header"));
}

#[test]
fn test_bat_previewer_handles_binary_files() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("binary.bin");
    let mut file = File::create(&file_path).unwrap();
    // Write some binary data with null bytes
    file.write_all(&[0, 1, 2, 3, 255, 0, 4, 5]).unwrap();

    let lines = build_preview_lines(&file_path, "");
    assert!(!lines.is_empty());
    let text = lines[0].to_string();
    assert!(text.contains("Binary File"));
}



