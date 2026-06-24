use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct CustomCommand {
    pub name: String,
    pub cmd: String,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SessionConfig {
    #[serde(default)]
    pub commands: Vec<CustomCommand>,
}

pub fn parse_session_config(workspace_root: &Path) -> SessionConfig {
    let config_path = workspace_root.join(".ascope.toml");
    if config_path.exists() {
        if let Ok(content) = fs::read_to_string(config_path) {
            if let Ok(cfg) = toml::from_str::<SessionConfig>(&content) {
                return cfg;
            }
        }
    }
    SessionConfig::default()
}
