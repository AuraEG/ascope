use std::fs;
use std::path::Path;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DetectedCommand {
    pub name: String,
    pub cmd: String,
    pub source: String,
}

pub fn detect_project_commands(path: &Path) -> Vec<DetectedCommand> {
    let mut commands = Vec::new();

    // 1. Rust / Cargo
    if path.join("Cargo.toml").exists() {
        commands.push(DetectedCommand {
            name: "cargo run".to_string(),
            cmd: "cargo run".to_string(),
            source: "Cargo".to_string(),
        });
        commands.push(DetectedCommand {
            name: "cargo build".to_string(),
            cmd: "cargo build".to_string(),
            source: "Cargo".to_string(),
        });
        commands.push(DetectedCommand {
            name: "cargo test".to_string(),
            cmd: "cargo test".to_string(),
            source: "Cargo".to_string(),
        });
        commands.push(DetectedCommand {
            name: "cargo clippy".to_string(),
            cmd: "cargo clippy --all-targets -- -D warnings".to_string(),
            source: "Cargo".to_string(),
        });
    }

    // 2. package.json scripts
    let package_json_path = path.join("package.json");
    if package_json_path.exists() {
        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
                    for (key, _) in scripts {
                        commands.push(DetectedCommand {
                            name: format!("npm run {}", key),
                            cmd: format!("npm run {}", key),
                            source: "package.json".to_string(),
                        });
                    }
                }
            }
        }
    }

    // 3. Makefile targets
    let makefile_path = path.join("Makefile");
    if makefile_path.exists() {
        if let Ok(content) = fs::read_to_string(&makefile_path) {
            for line in content.lines() {
                let line = line.trim();
                if let Some(colon_idx) = line.find(':') {
                    let target_part = line[..colon_idx].trim();
                    if !target_part.is_empty() 
                        && !target_part.starts_with('.') 
                        && !target_part.contains('=') 
                        && !target_part.contains('$') 
                        && target_part.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_')
                        && target_part != "PHONY" 
                    {
                        commands.push(DetectedCommand {
                            name: format!("make {}", target_part),
                            cmd: format!("make {}", target_part),
                            source: "Makefile".to_string(),
                        });
                    }
                }
            }
        }
    }

    commands
}
