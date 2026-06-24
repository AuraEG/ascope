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

#[test]
fn test_detect_pnpm_scripts() {
    let dir = tempdir().unwrap();
    let mut pkg = File::create(dir.path().join("package.json")).unwrap();
    write!(pkg, r#"{{"scripts": {{"build": "vite build"}}}}"#).unwrap();
    File::create(dir.path().join("pnpm-lock.yaml")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "pnpm run build"));
}

#[test]
fn test_detect_go_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("go.mod")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "go run ."));
    assert!(cmds.iter().any(|c| c.name == "go build"));
    assert!(cmds.iter().any(|c| c.name == "go test ./..."));
}

#[test]
fn test_detect_python_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("requirements.txt")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "python3 main.py"));
    assert!(cmds.iter().any(|c| c.name == "pip install -r requirements.txt"));
}

#[test]
fn test_detect_cmake_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("CMakeLists.txt")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "cmake build"));
}

#[test]
fn test_detect_java_maven_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("pom.xml")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "mvn package"));
    assert!(cmds.iter().any(|c| c.name == "mvn test"));
}

#[test]
fn test_detect_java_gradle_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("build.gradle")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "gradle build"));
    assert!(cmds.iter().any(|c| c.name == "gradle test"));
}

#[test]
fn test_detect_docker_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("Dockerfile")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "docker build"));
}

#[test]
fn test_detect_docker_compose_commands() {
    let dir = tempdir().unwrap();
    File::create(dir.path().join("docker-compose.yml")).unwrap();
    let cmds = detect_project_commands(dir.path());
    assert!(cmds.iter().any(|c| c.name == "docker compose up"));
    assert!(cmds.iter().any(|c| c.name == "docker compose down"));
}
