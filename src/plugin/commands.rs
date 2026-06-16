#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginCommand {
    ExecShell { cmd: String },
    FocusPath { path: String },
    None,
}
