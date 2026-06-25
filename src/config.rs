// ==========================================================================
// File    : config.rs
// Project : AuraScope
// Layer   : Core
// Purpose : Handles persistent bookmarks and recently visited directories
//           with cross-platform path resolution.
//
// Author  : Ahmed Ashour
// Created : 2026-06-14
// ==========================================================================

pub mod session;

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
pub struct Config {
    pub bookmarks: Vec<PathBuf>,
    pub recent: VecDeque<PathBuf>,
}

#[cfg(test)]
thread_local! {
    pub static TEST_CONFIG_PATH: std::cell::RefCell<Option<PathBuf>> = const { std::cell::RefCell::new(None) };
}

fn get_config_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())
            .or_else(|| std::env::var("HOME").ok())
            .map(|s| PathBuf::from(s).join("ascope"))
    } else {
        std::env::var("HOME")
            .ok()
            .or_else(|| std::env::var("USERPROFILE").ok())
            .map(|s| PathBuf::from(s).join(".config").join("ascope"))
    }
}

impl Config {
    pub fn get_path() -> Option<PathBuf> {
        #[cfg(test)]
        {
            if let Some(path) = TEST_CONFIG_PATH.with(|p| p.borrow().clone()) {
                return Some(path);
            }
            let thread_id = format!("{:?}", std::thread::current().id());
            let safe_id = thread_id.replace("ThreadId(", "").replace(")", "");
            return Some(PathBuf::from(format!(
                "target/test_bookmarks_{}.json",
                safe_id
            )));
        }
        #[allow(unreachable_code)]
        {
            if let Ok(custom_path) = std::env::var("ASCOPE_CONFIG_PATH") {
                return Some(PathBuf::from(custom_path));
            }
            get_config_dir().map(|d| d.join("bookmarks.json"))
        }
    }

    pub fn load() -> Self {
        if let Some(path) = Self::get_path() {
            if path.exists() {
                if let Ok(mut file) = File::open(&path) {
                    let mut data = String::new();
                    if file.read_to_string(&mut data).is_ok() {
                        if let Ok(config) = serde_json::from_str::<Config>(&data) {
                            let mut bookmarks = config.bookmarks;
                            bookmarks.dedup();
                            return Config {
                                bookmarks,
                                recent: config.recent,
                            };
                        }
                    }
                }
            }
        }
        Config::default()
    }

    pub fn save(&self) {
        if let Some(path) = Self::get_path() {
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Ok(data) = serde_json::to_string_pretty(self) {
                if let Ok(mut file) = File::create(&path) {
                    let _ = file.write_all(data.as_bytes());
                }
            }
        }
    }
}
