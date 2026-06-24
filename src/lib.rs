pub mod app;
pub mod config;
pub mod fs;
pub mod git;
pub mod navigation;
pub mod preview;
pub mod shell;
pub mod ui;
pub mod plugin {
    pub mod commands;
    pub mod engine;
    pub mod manifest;
}

pub mod search {
    pub mod ripgrep;
}

pub mod project {
    pub mod detector;
}
