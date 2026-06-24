use ascope::project::detector::detect_project_commands;
use tempfile::tempdir;
use std::fs::File;
use std::io::Write;

#[test]
fn test_detect_cargo_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("Cargo.toml")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "cargo build"));
    assert!(cmds.iter().any(|c| c.name == "cargo test"));
    assert!(cmds.iter().any(|c| c.name == "cargo run"));
}

#[test]
fn test_detect_npm_scripts() {
    let dir = tempdir().unwrap();
    let mut pkg = File::create(dir.path().join("package.json")).unwrap();
    write!(pkg, r#"{{"scripts": {{"start": "node index.js", "test": "jest"}}}}"#).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "npm run start"));
    assert!(cmds.iter().any(|c| c.name == "npm run test"));
}

#[test]
fn test_detect_makefile_targets() {
    let dir = tempdir().unwrap();
    let mut mf = File::create(dir.path().join("Makefile")).unwrap();
    writeln!(mf, ".PHONY: all build test").unwrap();
    writeln!(mf, "all: build").unwrap();
    writeln!(mf, "build:").unwrap();
    writeln!(mf, "\tcargo build").unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "make build"));
    assert!(!cmds.iter().any(|c| c.name == "make PHONY"));
}
