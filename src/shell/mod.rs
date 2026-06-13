// ==========================================================================
// File    : shell/mod.rs
// Project : AuraScope
// Layer   : Shell
// Purpose : Shell integration utilities and wrapper assets for exporting the
//           active TUI directory back to the parent shell.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

use std::path::Path;

// --------------------------------------------------------------------------
// [SECTION] Public API
// --------------------------------------------------------------------------

/// Persist the final TUI directory into `export_file` so a parent shell wrapper
/// can read it and perform the actual `cd` in the caller's session.
pub fn write_export_target(export_file: &Path, selected_path: &Path) -> std::io::Result<()> {
    std::fs::write(export_file, selected_path.display().to_string())
}

// --------------------------------------------------------------------------
// [SECTION] Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use tempfile::NamedTempFile;

    #[test]
    fn test_write_export_target_writes_selected_path() {
        let temp = NamedTempFile::new().unwrap();
        let target = PathBuf::from("/tmp/ascope-demo");

        write_export_target(temp.path(), &target).unwrap();

        let content = std::fs::read_to_string(temp.path()).unwrap();
        assert_eq!(content, target.display().to_string());
    }
}
