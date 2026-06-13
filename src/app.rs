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

use jwalk::WalkDir;

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
    /// Index of the highlighted row in the currently visible item set.
    pub selected_index: usize,
    /// Whether the bottom search overlay is currently focused.
    pub search_mode: bool,
    /// Current live substring query used to filter visible rows.
    pub search_query: String,
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
            search_mode: false,
            search_query: String::new(),
        }
    }

    /// Return the currently visible items, filtered when search is active.
    pub fn visible_items(&self) -> Vec<(PathBuf, u64)> {
        if self.search_query.is_empty() {
            self.items.clone()
        } else {
            self.filter_query(&self.search_query)
        }
    }

    /// Return the currently selected visible entry, if any.
    pub fn selected_item(&self) -> Option<(PathBuf, u64)> {
        let visible = self.visible_items();
        if visible.is_empty() {
            return None;
        }
        let index = self.selected_index.min(visible.len() - 1);
        visible.get(index).cloned()
    }

    /// Descend into the currently selected sub-directory and rescan.
    pub fn navigate_in(&mut self) {
        if let Some((target, _)) = self.selected_item() {
            if target.is_dir() {
                *self = Self::new(target);
            }
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
        let len = self.visible_items().len();
        if len == 0 {
            return;
        }
        // Up from index 0 wraps to the bottom; down from the last wraps to the top.
        self.selected_index = match delta {
            d if d > 0 => (self.selected_index + 1) % len,
            _ => self.selected_index.checked_sub(1).unwrap_or(len - 1),
        };
    }

    /// Toggle search-mode focus. Leaving search mode preserves the query so the
    /// user can inspect the filtered results before clearing it.
    pub fn toggle_search_mode(&mut self) {
        self.search_mode = !self.search_mode;
    }

    /// Clear the active query and reset the cursor to the first visible row.
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.selected_index = 0;
    }

    /// Append one typed character to the live query.
    pub fn push_search_char(&mut self, ch: char) {
        self.search_query.push(ch);
        self.selected_index = 0;
    }

    /// Delete the most recent search character.
    pub fn pop_search_char(&mut self) {
        self.search_query.pop();
        self.selected_index = 0;
    }

    /// Case-insensitive substring match over files and directories inside the
    /// active path. Search is recursive so file previews can be opened from the
    /// live overlay without navigating into every intermediate directory.
    pub fn filter_query(&self, query: &str) -> Vec<(PathBuf, u64)> {
        let q = query.to_lowercase();
        let mut matches = Vec::new();

        for entry in WalkDir::new(&self.current_path)
            .skip_hidden(true)
            .into_iter()
            .filter_map(Result::ok)
        {
            let path = entry.path();
            if path == self.current_path {
                continue;
            }

            if path.to_string_lossy().to_lowercase().contains(&q) {
                let size = entry
                    .metadata()
                    .map(|metadata| {
                        if metadata.is_file() {
                            metadata.len()
                        } else {
                            0
                        }
                    })
                    .unwrap_or(0);
                matches.push((path.to_path_buf(), size));
            }
        }

        matches.sort_by(|a, b| a.0.cmp(&b.0));
        matches
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

    #[test]
    fn test_live_filter_items() {
        let mut state = AppState::new(PathBuf::from("."));
        state.items = vec![
            (PathBuf::from("Cargo.toml"), 100),
            (PathBuf::from("src/main.rs"), 500),
            (PathBuf::from("src/app.rs"), 300),
        ];

        let filtered = state.filter_query("main");
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].0.ends_with(PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_visible_items_uses_search_query() {
        let mut state = AppState::new(PathBuf::from("."));
        state.items = vec![
            (PathBuf::from("Cargo.toml"), 100),
            (PathBuf::from("src/main.rs"), 500),
            (PathBuf::from("src/app.rs"), 300),
        ];
        state.search_query = String::from("app");

        let visible = state.visible_items();
        assert_eq!(visible.len(), 1);
        assert!(visible[0].0.ends_with(PathBuf::from("src/app.rs")));
    }

    #[test]
    fn test_search_editing_resets_selection() {
        let mut state = AppState::new(PathBuf::from("."));
        state.items = vec![(PathBuf::from("alpha"), 1), (PathBuf::from("beta"), 1)];
        state.selected_index = 1;

        state.push_search_char('a');
        assert_eq!(state.selected_index, 0);
        assert_eq!(state.search_query, "a");

        state.pop_search_char();
        assert_eq!(state.selected_index, 0);
        assert!(state.search_query.is_empty());
    }

    #[test]
    fn test_filter_query_matches_nested_files() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir(&src_dir).unwrap();
        let file_path = src_dir.join("main.rs");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"fn main() {}\n").unwrap();

        let state = AppState::new(dir.path().to_path_buf());
        let filtered = state.filter_query("main.rs");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0, file_path);
    }
}
