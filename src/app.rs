// ==========================================================================
// File    : app.rs
// Project : AuraScope
// Layer   : Core
// Purpose : Application state machine; owns the navigator cursor and the
//           active scan results fed into the TUI renderer each frame.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

use std::cmp::Reverse;
use std::path::PathBuf;

use crate::fs::walker::{scan_path, PathStats};

// --------------------------------------------------------------------------
// [SECTION] State
// --------------------------------------------------------------------------

/// Central state passed to every render call and mutated by keyboard events.
pub struct AppState {
    /// Directory currently displayed in the tree pane.
    pub current_path: PathBuf,
    /// Aggregated scan results for `current_path`.
    pub active_stats: PathStats,
    /// Direct child directories sorted by size descending; drives the list.
    pub items: Vec<(PathBuf, u64)>,
    /// Index of the highlighted row in `items`.
    pub selected_index: usize,
}

// --------------------------------------------------------------------------
// [SECTION] State Machine
// --------------------------------------------------------------------------

impl AppState {
    /// Scan `root` and build initial state. Falls back to empty stats if the
    /// path is unreadable so the TUI can still open without crashing.
    pub fn new(root: PathBuf) -> Self {
        let stats = scan_path(&root).unwrap_or_default();
        let mut items: Vec<(PathBuf, u64)> = stats.subdirs.clone().into_iter().collect();
        // Largest directories first so the user immediately spots the disk hogs.
        items.sort_by_key(|x| Reverse(x.1));

        Self {
            current_path: root,
            active_stats: stats,
            items,
            selected_index: 0,
        }
    }

    /// Descend into the currently selected sub-directory and rescan.
    pub fn navigate_in(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let target = self.items[self.selected_index].0.clone();
        if target.is_dir() {
            *self = Self::new(target);
        }
    }

    /// Ascend to the parent directory and rescan.
    pub fn navigate_out(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            *self = Self::new(parent.to_path_buf());
        }
    }

    /// Move the selection cursor by `delta` rows, wrapping at both ends.
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.items.len();
        if len == 0 {
            return;
        }
        // Up from index 0 wraps to the bottom; down from the last wraps to the top.
        self.selected_index = match delta {
            d if d > 0 => (self.selected_index + 1) % len,
            _ => self.selected_index.checked_sub(1).unwrap_or(len - 1),
        };
    }
}

// --------------------------------------------------------------------------
// [SECTION] Tests
// --------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;

    /// Build a temp dir with three named sub-dirs, each containing one file.
    fn make_state_with_subdirs() -> (AppState, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        for name in &["alpha", "beta", "gamma"] {
            let sub = dir.path().join(name);
            std::fs::create_dir(&sub).unwrap();
            let mut f = File::create(sub.join("data.bin")).unwrap();
            f.write_all(b"payload").unwrap();
        }
        let state = AppState::new(dir.path().to_path_buf());
        (state, dir)
    }

    #[test]
    fn test_new_state_has_items() {
        let (state, _dir) = make_state_with_subdirs();
        assert_eq!(state.items.len(), 3);
        assert_eq!(state.selected_index, 0);
    }

    #[test]
    fn test_move_selection_wraps_forward() {
        let (mut state, _dir) = make_state_with_subdirs();
        state.selected_index = 2; // last item
        state.move_selection(1);
        assert_eq!(state.selected_index, 0); // must wrap to first
    }

    #[test]
    fn test_move_selection_wraps_backward() {
        let (mut state, _dir) = make_state_with_subdirs();
        state.selected_index = 0;
        state.move_selection(-1);
        assert_eq!(state.selected_index, 2); // must wrap to last
    }

    #[test]
    fn test_navigate_out_does_not_panic() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());
        // Ascending from a temp dir must not panic regardless of depth.
        state.navigate_out();
    }
}
