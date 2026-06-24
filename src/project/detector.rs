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

    // 2. Node.js / npm / pnpm / bun / yarn
    let package_json_path = path.join("package.json");
    if package_json_path.exists() {
        let pm = if path.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if path.join("bun.lockb").exists() || path.join("bun.lock").exists() {
            "bun"
        } else if path.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm"
        };

        if let Ok(content) = fs::read_to_string(&package_json_path) {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&content) {
                if let Some(scripts) = val.get("scripts").and_then(|s| s.as_object()) {
                    for (key, _) in scripts {
                        let cmd_str = match pm {
                            "yarn" => format!("yarn {}", key),
                            "bun" => format!("bun run {}", key),
                            "pnpm" => format!("pnpm run {}", key),
                            _ => format!("npm run {}", key),
                        };
                        commands.push(DetectedCommand {
                            name: cmd_str.clone(),
                            cmd: cmd_str,
                            source: format!("package.json ({})", pm),
                        });
                    }
                }
            }
        }
    }

    // 3. Makefile
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

    // 4. Go
    if path.join("go.mod").exists() {
        commands.push(DetectedCommand {
            name: "go run .".to_string(),
            cmd: "go run .".to_string(),
            source: "Go".to_string(),
        });
        commands.push(DetectedCommand {
            name: "go build".to_string(),
            cmd: "go build".to_string(),
            source: "Go".to_string(),
        });
        commands.push(DetectedCommand {
            name: "go test ./...".to_string(),
            cmd: "go test ./...".to_string(),
            source: "Go".to_string(),
        });
        commands.push(DetectedCommand {
            name: "go vet".to_string(),
            cmd: "go vet ./...".to_string(),
            source: "Go".to_string(),
        });
    }

    // 5. Python
    if path.join("requirements.txt").exists() 
        || path.join("pyproject.toml").exists() 
        || path.join("setup.py").exists() 
        || path.join("Pipfile").exists() 
    {
        commands.push(DetectedCommand {
            name: "python3 main.py".to_string(),
            cmd: "python3 main.py".to_string(),
            source: "Python".to_string(),
        });
        if path.join("requirements.txt").exists() {
            commands.push(DetectedCommand {
                name: "pip install -r requirements.txt".to_string(),
                cmd: "pip install -r requirements.txt".to_string(),
                source: "Python".to_string(),
            });
        }
        let pyproject_path = path.join("pyproject.toml");
        if pyproject_path.exists() {
            if let Ok(c) = fs::read_to_string(&pyproject_path) {
                if c.contains("[tool.poetry]") {
                    commands.push(DetectedCommand {
                        name: "poetry run python main.py".to_string(),
                        cmd: "poetry run python main.py".to_string(),
                        source: "Python (Poetry)".to_string(),
                    });
                    commands.push(DetectedCommand {
                        name: "poetry install".to_string(),
                        cmd: "poetry install".to_string(),
                        source: "Python (Poetry)".to_string(),
                    });
                }
            }
        }
    }

    // 6. C/C++ (CMake)
    if path.join("CMakeLists.txt").exists() {
        commands.push(DetectedCommand {
            name: "cmake build".to_string(),
            cmd: "cmake -B build && cmake --build build".to_string(),
            source: "CMake".to_string(),
        });
    }

    // 7. Java (Maven & Gradle)
    if path.join("pom.xml").exists() {
        commands.push(DetectedCommand {
            name: "mvn package".to_string(),
            cmd: "mvn clean package".to_string(),
            source: "Maven".to_string(),
        });
        commands.push(DetectedCommand {
            name: "mvn test".to_string(),
            cmd: "mvn test".to_string(),
            source: "Maven".to_string(),
        });
    }
    if path.join("build.gradle").exists() || path.join("build.gradle.kts").exists() {
        let gradlew = if path.join("gradlew").exists() { "./gradlew" } else { "gradle" };
        commands.push(DetectedCommand {
            name: format!("{} build", gradlew),
            cmd: format!("{} build", gradlew),
            source: "Gradle".to_string(),
        });
        commands.push(DetectedCommand {
            name: format!("{} test", gradlew),
            cmd: format!("{} test", gradlew),
            source: "Gradle".to_string(),
        });
    }

    // 8. Docker & Docker Compose
    if path.join("Dockerfile").exists() {
        commands.push(DetectedCommand {
            name: "docker build".to_string(),
            cmd: "docker build -t app .".to_string(),
            source: "Docker".to_string(),
        });
    }
    if path.join("docker-compose.yml").exists() || path.join("docker-compose.yaml").exists() {
        commands.push(DetectedCommand {
            name: "docker compose up".to_string(),
            cmd: "docker compose up -d".to_string(),
            source: "Docker Compose".to_string(),
        });
        commands.push(DetectedCommand {
            name: "docker compose down".to_string(),
            cmd: "docker compose down".to_string(),
            source: "Docker Compose".to_string(),
        });
    }

    commands
}
