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
use std::sync::{Arc, Mutex};

use jwalk::WalkDir;

use crate::fs::walker::{scan_path_async, PathStats, ScanProgress};
use ratatui::text::Line;

// --------------------------------------------------------------------------
// [SECTION] State
// --------------------------------------------------------------------------

/// Central state passed to every render call and mutated by keyboard events.
pub struct AppState {
    /// Directory currently displayed in the tree pane.
    pub current_path: PathBuf,
    /// Aggregated scan results for `current_path` (shared with scanner thread).
    active_stats: Arc<Mutex<PathStats>>,
    /// Lifecycle of the background scan for `current_path`.
    scan_progress: Arc<Mutex<ScanProgress>>,
    /// Direct child directories sorted by size descending; drives the list.
    pub items: Vec<(PathBuf, u64)>,
    /// Index of the highlighted row in the currently visible item set.
    pub selected_index: usize,
    /// Whether the bottom search overlay is currently focused.
    pub search_mode: bool,
    /// Current live substring query used to filter visible rows.
    pub search_query: String,
    /// Flag indicating if the background scan results have already been applied to `items`.
    scan_applied: bool,
    /// Cache for the highlighted preview lines of the currently selected file.
    preview_cache: Option<(PathBuf, Vec<Line<'static>>)>,
}

// --------------------------------------------------------------------------
// [SECTION] State Machine
// --------------------------------------------------------------------------

impl AppState {
    /// Start an async scan of `root` and return immediately with empty stats.
    /// The TUI renders right away; `poll_scan()` will populate `items` once
    /// the background thread finishes.
    pub fn new(root: PathBuf) -> Self {
        let active_stats = Arc::new(Mutex::new(PathStats::default()));
        let scan_progress = Arc::new(Mutex::new(ScanProgress::default()));

        scan_path_async(
            root.clone(),
            Arc::clone(&active_stats),
            Arc::clone(&scan_progress),
        );

        Self {
            current_path: root,
            active_stats,
            scan_progress,
            items: Vec::new(),
            selected_index: 0,
            search_mode: false,
            search_query: String::new(),
            scan_applied: false,
            preview_cache: None,
        }
    }

    // ------------------------------------------------------------------
    // [SECTION] Async Scan Helpers
    // ------------------------------------------------------------------

    /// Returns `true` while the background scanner thread is still running.
    pub fn is_scanning(&self) -> bool {
        matches!(
            *self.scan_progress.lock().unwrap_or_else(|e| e.into_inner()),
            ScanProgress::Scanning
        )
    }

    /// Returns a spinner label while scanning, or an empty string when done.
    /// Available as a public API for widgets that need explicit spinner text.
    pub fn scan_progress_label(&self) -> String {
        if self.is_scanning() {
            "⣾ Scanning…".to_string()
        } else {
            String::new()
        }
    }

    /// Returns the total size of the current scanned directory tree.
    pub fn total_size(&self) -> u64 {
        self.active_stats
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .total_size
    }

    /// Called every render frame. If the scan has just completed, rebuild
    /// `items` from the freshly populated stats so the tree pane updates.
    pub fn poll_scan(&mut self) {
        if !self.scan_applied
            && matches!(
                *self.scan_progress.lock().unwrap_or_else(|e| e.into_inner()),
                ScanProgress::Complete
            )
        {
            let stats = self.active_stats.lock().unwrap_or_else(|e| e.into_inner());
            let mut new_items: Vec<(PathBuf, u64)> = stats.subdirs.clone().into_iter().collect();
            // Largest directories first so the user immediately spots disk hogs.
            new_items.sort_by_key(|x| Reverse(x.1));
            drop(stats);
            self.items = new_items;
            self.scan_applied = true;
        }
    }

    /// Check if the currently highlighted item is a file and update the
    /// preview cache if the selected file has changed.
    pub fn update_preview_cache(&mut self) {
        let selected = self.selected_item().map(|x| x.0);

        if let Some((cached_path, _)) = &self.preview_cache {
            if Some(cached_path) == selected.as_ref() {
                return;
            }
        }

        if let Some(path) = selected {
            if path.is_file() {
                let lines = crate::ui::widgets::build_preview_lines(&path);
                self.preview_cache = Some((path, lines));
                return;
            }
        }

        self.preview_cache = None;
    }

    /// Access the cached highlighted lines of the currently selected file.
    pub fn preview_lines(&self) -> &[Line<'static>] {
        if let Some((_, lines)) = &self.preview_cache {
            lines
        } else {
            &[]
        }
    }

    // ------------------------------------------------------------------
    // [SECTION] Navigation & Selection
    // ------------------------------------------------------------------

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

    /// Wait for a newly-created `AppState` to finish its background scan so
    /// that tests can make assertions on `items`.
    fn wait_for_scan(state: &mut AppState) {
        let start = std::time::Instant::now();
        loop {
            state.poll_scan();
            if matches!(
                *state
                    .scan_progress
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
                ScanProgress::Complete
            ) {
                break;
            }
            assert!(start.elapsed().as_secs() < 5, "scan timed out");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // One final poll to make sure items are populated after completion.
        state.poll_scan();
    }

    /// Build a temp dir with three named sub-dirs, each containing one file.
    fn make_state_with_subdirs() -> (AppState, tempfile::TempDir) {
        let dir = tempdir().unwrap();
        for name in &["alpha", "beta", "gamma"] {
            let sub = dir.path().join(name);
            std::fs::create_dir(&sub).unwrap();
            let mut f = File::create(sub.join("data.bin")).unwrap();
            f.write_all(b"payload").unwrap();
        }
        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);
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

    #[test]
    fn test_is_scanning_and_poll_scan() {
        let dir = tempdir().unwrap();
        let mut f = File::create(dir.path().join("x.txt")).unwrap();
        f.write_all(b"data").unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        // The scan starts asynchronously. We don't assert the transient Scanning
        // state because it may already be Complete before this thread is scheduled.
        // We only assert the deterministic post-wait outcome.
        wait_for_scan(&mut state);
        assert!(!state.is_scanning(), "scan must not be Scanning after wait");
        assert!(
            matches!(
                *state
                    .scan_progress
                    .lock()
                    .unwrap_or_else(|e| e.into_inner()),
                ScanProgress::Complete
            ),
            "scan must be Complete after wait_for_scan"
        );
    }

    #[test]
    fn test_poll_scan_files_only() {
        let dir = tempdir().unwrap();
        // Create only a file, no subdirectories
        let mut f = File::create(dir.path().join("only_file.txt")).unwrap();
        f.write_all(b"data").unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        // Should be complete, and scan_applied should be true
        assert!(!state.is_scanning());
        assert!(state.items.is_empty());
        assert!(state.scan_applied);

        // Subsequent polls should be no-ops and not lock indefinitely
        state.poll_scan();
    }

    #[test]
    fn test_preview_cache_updates_and_caches() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("code.rs");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"fn test() {}\n").unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        // Set state items explicitly for cursor movement/selection testing
        state.items = vec![(file_path.clone(), 12)];
        state.selected_index = 0;

        // Verify initially cache is empty
        assert!(state.preview_lines().is_empty());

        // Update preview cache
        state.update_preview_cache();

        // Check if cache contains the selected file lines
        let lines = state.preview_lines();
        assert!(!lines.is_empty());
        assert!(lines[0].to_string().contains("fn test()"));

        // Change selection to None (or clear items) and check that cache is cleared
        state.items.clear();
        state.update_preview_cache();
        assert!(state.preview_lines().is_empty());
    }
}
