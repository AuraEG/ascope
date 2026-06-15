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

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::fs::walker::{scan_path_async, PathStats, ScanProgress};
use crate::git::GitContext;
use nucleo::{
    pattern::{CaseMatching, Normalization, Pattern},
    Config, Matcher,
};
use ratatui::text::Line;

// --------------------------------------------------------------------------
// [SECTION] State
// --------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortMode {
    SizeDesc,
    NameAsc,
    MtimeDesc,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalMode {
    None,
    Bookmarks,
    Recent,
    OpenConfirmation,
    DeleteConfirmation,
}

use crate::fs::walker::{DirEntry, EntryType};

/// Represents a single directory view tab with its own navigation history and search query.
pub struct Tab {
    pub current_path: PathBuf,
    pub active_stats: Arc<Mutex<PathStats>>,
    pub scan_progress: Arc<Mutex<ScanProgress>>,
    pub navigation: crate::navigation::Navigation,
    pub all_entries: Vec<DirEntry>,
    pub scan_applied: bool,
    pub preview_cache: Option<(PathBuf, String, Vec<Line<'static>>)>,
    pub git_ctx: Option<GitContext>,
}

/// Central state passed to every render call and mutated by keyboard events.
pub struct AppState {
    pub current_path: PathBuf,
    active_stats: Arc<Mutex<PathStats>>,
    scan_progress: Arc<Mutex<ScanProgress>>,
    pub navigation: crate::navigation::Navigation,
    pub all_entries: Vec<DirEntry>,
    scan_applied: bool,
    preview_cache: Option<(PathBuf, String, Vec<Line<'static>>)>,
    pub git_ctx: Option<GitContext>,
    pub search_mode: bool,
    pub tabs: Vec<Tab>,
    /// Index of the active tab
    pub active_tab: usize,
    /// Persistent bookmark configuration
    pub config: crate::config::Config,
    /// Current open modal type
    pub modal_mode: ModalMode,
    /// Highlighted cursor index inside the modal
    pub modal_selected_index: usize,
    /// Input buffer for typing number to select modal entries
    pub modal_input: String,
    /// Notification toast message with timestamp
    pub notification: Option<(String, std::time::Instant)>,
    /// Temp target path for tab open confirmation
    pub modal_target_path: Option<PathBuf>,
    /// Previous modal mode before confirmation popup
    pub modal_confirm_prev: ModalMode,
    /// Whether to open confirmation target path in a new tab
    pub modal_confirm_new_tab: bool,
    /// Selected paths for multi-file operations
    pub selected_paths: std::collections::HashSet<PathBuf>,
    /// Yanked paths for pasting
    pub yanked_files: Vec<PathBuf>,
    /// Cut paths for pasting
    pub cut_files: Vec<PathBuf>,
    /// Target path for renaming
    pub rename_target: Option<PathBuf>,
    /// Text input buffer for renaming
    pub rename_input: String,
    /// Whether inline rename mode is active
    pub rename_mode: bool,
    /// Target paths for deletion confirmation
    pub delete_targets: Vec<PathBuf>,
    /// Whether full-screen help modal is open
    pub show_help: bool,
    /// Selected index inside the help modal
    pub help_selected_index: usize,
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

        let git_ctx = GitContext::read(&root);

        let mut config = crate::config::Config::load();
        // Skip adding "." or empty paths to recent if we want to resolve to absolute, but keeping it simple and direct is standard.
        // Wait, to make it completely correct and consistent, let's normalize or use it directly. Using directly is perfect.
        if !config.recent.contains(&root) {
            config.recent.push_front(root.clone());
            if config.recent.len() > 50 {
                config.recent.pop_back();
            }
            config.save();
        }

        let initial_tab = Tab {
            current_path: root.clone(),
            active_stats: Arc::clone(&active_stats),
            scan_progress: Arc::clone(&scan_progress),
            navigation: crate::navigation::Navigation::new(Vec::new(), SortMode::SizeDesc),
            all_entries: Vec::new(),
            scan_applied: false,
            preview_cache: None,
            git_ctx: git_ctx.clone(),
        };

        Self {
            current_path: root,
            active_stats,
            scan_progress,
            navigation: crate::navigation::Navigation::new(Vec::new(), SortMode::SizeDesc),
            all_entries: Vec::new(),
            scan_applied: false,
            preview_cache: None,
            git_ctx,
            search_mode: false,
            tabs: vec![initial_tab],
            active_tab: 0,
            config,
            modal_mode: ModalMode::None,
            modal_selected_index: 0,
            modal_input: String::new(),
            notification: None,
            modal_target_path: None,
            modal_confirm_prev: ModalMode::None,
            modal_confirm_new_tab: false,
            selected_paths: std::collections::HashSet::new(),
            yanked_files: Vec::new(),
            cut_files: Vec::new(),
            rename_target: None,
            rename_input: String::new(),
            rename_mode: false,
            delete_targets: Vec::new(),
            show_help: false,
            help_selected_index: 0,
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
            let new_items: Vec<DirEntry> = stats
                .all_entries
                .iter()
                .filter(|entry| entry.path.parent() == Some(&self.current_path))
                .cloned()
                .collect();
            self.all_entries = stats.all_entries.clone();
            drop(stats);
            self.navigation.update_items(new_items);
            self.scan_applied = true;
        }
    }

    /// Cycle through size -> name -> modification time sorting modes.
    pub fn cycle_sort_mode(&mut self) {
        let next_mode = match self.navigation.sort_mode() {
            SortMode::SizeDesc => SortMode::NameAsc,
            SortMode::NameAsc => SortMode::MtimeDesc,
            SortMode::MtimeDesc => SortMode::SizeDesc,
        };
        self.navigation.set_sort_mode(next_mode);
    }

    /// Check if the currently highlighted item is a file and update the
    /// preview cache if the selected file or search query has changed.
    pub fn update_preview_cache(&mut self) {
        let selected = self.selected_item().map(|x| x.path);
        let query = self.navigation.filter_query().unwrap_or("").to_string();

        if let Some((cached_path, cached_query, _)) = &self.preview_cache {
            if Some(cached_path) == selected.as_ref() && cached_query == &query {
                return;
            }
        }

        if let Some(path) = selected {
            if path.is_file() {
                let lines = crate::ui::widgets::build_preview_lines(&path, &query);
                self.preview_cache = Some((path, query, lines));
                return;
            }
        }

        self.preview_cache = None;
    }

    /// Access the cached highlighted lines of the currently selected file.
    pub fn preview_lines(&self) -> &[Line<'static>] {
        if let Some((_, _, lines)) = &self.preview_cache {
            lines
        } else {
            &[]
        }
    }

    // ------------------------------------------------------------------
    // [SECTION] Navigation & Selection
    // ------------------------------------------------------------------

    /// Return the currently visible items, filtered when search is active.
    pub fn visible_items(&self) -> Vec<(DirEntry, u32)> {
        if self.navigation.filter_query().is_none() {
            return self.build_expanded_tree();
        }

        self.navigation
            .visible_items_with_scores()
            .into_iter()
            .map(|(entry, score)| (entry.clone(), score))
            .collect()
    }

    /// Return the currently selected visible entry, if any.
    pub fn selected_item(&self) -> Option<DirEntry> {
        self.navigation.current_selection().cloned()
    }

    /// Toggle the expansion state of the currently selected directory.
    pub fn toggle_expand(&mut self) {
        self.navigation.toggle_expand_selected();
    }

    fn get_children(&self, parent_path: &std::path::Path) -> Vec<DirEntry> {
        let mut children: Vec<DirEntry> = self
            .all_entries
            .iter()
            .filter(|e| e.path.parent() == Some(parent_path))
            .cloned()
            .collect();

        // Sort children using the active sorting mode
        match self.navigation.sort_mode() {
            SortMode::SizeDesc => {
                children.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
            }
            SortMode::NameAsc => {
                children.sort_by(|a, b| {
                    let name_a = a.path.file_name().unwrap_or_default();
                    let name_b = b.path.file_name().unwrap_or_default();
                    name_a.cmp(name_b)
                });
            }
            SortMode::MtimeDesc => {
                children.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
            }
        }
        children
    }

    fn build_expanded_tree(&self) -> Vec<(DirEntry, u32)> {
        let mut result = Vec::new();
        // Start with top-level items from Navigation (which are already sorted)
        let mut stack: Vec<(DirEntry, usize)> =
            self.navigation.visible_items().iter().rev().map(|&e| (e.clone(), 0)).collect();

        while let Some((entry, depth)) = stack.pop() {
            result.push((entry.clone(), 0));
            if entry.entry_type == EntryType::Directory && self.navigation.is_expanded(&entry.path)
            {
                // Get children of this directory, sorted in reverse order so that when popped from stack they come out in correct order
                let mut children = self.get_children(&entry.path);
                children.reverse();
                for child in children {
                    stack.push((child, depth + 1));
                }
            }
        }
        result
    }

    /// Descend into the currently selected sub-directory and rescan inside the active tab.
    pub fn navigate_in(&mut self) {
        if let Some(target) = self.selected_item() {
            if target.entry_type == EntryType::Directory {
                let path = target.path;
                let active_stats = Arc::new(Mutex::new(PathStats::default()));
                let scan_progress = Arc::new(Mutex::new(ScanProgress::default()));

                scan_path_async(
                    path.clone(),
                    Arc::clone(&active_stats),
                    Arc::clone(&scan_progress),
                );

                let git_ctx = GitContext::read(&path);

                self.current_path = path.clone();
                self.active_stats = active_stats;
                self.scan_progress = scan_progress;
                self.navigation.update_items(Vec::new());
                self.navigation.set_cursor(0);
                self.navigation.clear_expanded();
                self.navigation.set_filter(None);
                self.all_entries = Vec::new();
                self.scan_applied = false;
                self.preview_cache = None;
                self.git_ctx = git_ctx;

                self.record_navigation(path);
                self.save_active_tab();
            }
        }
    }

    /// Ascend to the parent directory and rescan inside the active tab.
    pub fn navigate_out(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            let path = parent.to_path_buf();
            let active_stats = Arc::new(Mutex::new(PathStats::default()));
            let scan_progress = Arc::new(Mutex::new(ScanProgress::default()));

            scan_path_async(
                path.clone(),
                Arc::clone(&active_stats),
                Arc::clone(&scan_progress),
            );

            let git_ctx = GitContext::read(&path);

            self.current_path = path.clone();
            self.active_stats = active_stats;
            self.scan_progress = scan_progress;
            self.navigation.update_items(Vec::new());
            self.navigation.set_cursor(0);
            self.navigation.clear_expanded();
            self.navigation.set_filter(None);
            self.all_entries = Vec::new();
            self.scan_applied = false;
            self.preview_cache = None;
            self.git_ctx = git_ctx;

            self.record_navigation(path);
            self.save_active_tab();
        }
    }

    /// Save the active state values into the corresponding Tab struct inside self.tabs.
    pub fn save_active_tab(&mut self) {
        if self.active_tab < self.tabs.len() {
            self.tabs[self.active_tab] = Tab {
                current_path: self.current_path.clone(),
                active_stats: Arc::clone(&self.active_stats),
                scan_progress: Arc::clone(&self.scan_progress),
                navigation: self.navigation.clone(),
                all_entries: self.all_entries.clone(),
                scan_applied: self.scan_applied,
                preview_cache: self.preview_cache.clone(),
                git_ctx: self.git_ctx.clone(),
            };
        }
    }

    /// Load the specified tab index state values into the AppState active fields.
    pub fn load_tab(&mut self, index: usize) {
        if index < self.tabs.len() {
            let tab = &self.tabs[index];
            self.current_path = tab.current_path.clone();
            self.active_stats = Arc::clone(&tab.active_stats);
            self.scan_progress = Arc::clone(&tab.scan_progress);
            self.navigation = tab.navigation.clone();
            self.all_entries = tab.all_entries.clone();
            self.scan_applied = tab.scan_applied;
            self.preview_cache = tab.preview_cache.clone();
            self.git_ctx = tab.git_ctx.clone();
            self.active_tab = index;
        }
    }

    /// Open a new tab at the specified path and switch to it.
    pub fn open_tab(&mut self, path: PathBuf) {
        self.save_active_tab();

        let active_stats = Arc::new(Mutex::new(PathStats::default()));
        let scan_progress = Arc::new(Mutex::new(ScanProgress::default()));

        scan_path_async(
            path.clone(),
            Arc::clone(&active_stats),
            Arc::clone(&scan_progress),
        );

        let git_ctx = GitContext::read(&path);

        let new_tab = Tab {
            current_path: path,
            active_stats,
            scan_progress,
            navigation: crate::navigation::Navigation::new(Vec::new(), SortMode::SizeDesc),
            all_entries: Vec::new(),
            scan_applied: false,
            preview_cache: None,
            git_ctx,
        };

        self.tabs.push(new_tab);
        let new_idx = self.tabs.len() - 1;
        self.load_tab(new_idx);
    }

    /// Open a new tab at the home directory and switch to it.
    pub fn open_home_tab(&mut self) {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("/"));
        self.open_tab(home);
    }

    /// Close the active tab and switch to another tab. Reject if it is the last tab.
    pub fn close_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.tabs.remove(self.active_tab);
        let new_idx = if self.active_tab >= self.tabs.len() {
            self.tabs.len() - 1
        } else {
            self.active_tab
        };
        self.load_tab(new_idx);
    }

    /// Cycle to the next tab.
    pub fn next_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.save_active_tab();
        let new_idx = (self.active_tab + 1) % self.tabs.len();
        self.load_tab(new_idx);
    }

    /// Cycle to the previous tab.
    pub fn prev_tab(&mut self) {
        if self.tabs.len() <= 1 {
            return;
        }
        self.save_active_tab();
        let new_idx = (self.active_tab + self.tabs.len() - 1) % self.tabs.len();
        self.load_tab(new_idx);
    }

    /// Record a newly navigated directory in the recently visited list.
    pub fn record_navigation(&mut self, path: PathBuf) {
        if !self.config.recent.contains(&path) {
            self.config.recent.push_front(path);
            if self.config.recent.len() > 50 {
                self.config.recent.pop_back();
            }
            self.config.save();
        }
    }

    /// Add the active tab's current path to the bookmarks list.
    pub fn add_bookmark(&mut self) {
        let path = self.current_path.clone();
        if !self.config.bookmarks.contains(&path) {
            self.config.bookmarks.push(path);
            self.config.save();
            let name = self
                .current_path
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| self.current_path.to_string_lossy().into_owned());
            self.notification = Some((
                format!("Added to Bookmarks: {}", name),
                std::time::Instant::now(),
            ));
        }
    }

    /// Remove a bookmark by index.
    pub fn remove_bookmark(&mut self, index: usize) {
        if index < self.config.bookmarks.len() {
            self.config.bookmarks.remove(index);
            self.config.save();
            // Clamp modal selection index
            if self.config.bookmarks.is_empty() {
                self.modal_selected_index = 0;
            } else if self.modal_selected_index >= self.config.bookmarks.len() {
                self.modal_selected_index = self.config.bookmarks.len() - 1;
            }
        }
    }

    /// Remove a recently visited path by index.
    pub fn remove_recent(&mut self, index: usize) {
        if index < self.config.recent.len() {
            self.config.recent.remove(index);
            self.config.save();
            // Clamp modal selection index
            if self.config.recent.is_empty() {
                self.modal_selected_index = 0;
            } else if self.modal_selected_index >= self.config.recent.len() {
                self.modal_selected_index = self.config.recent.len() - 1;
            }
        }
    }

    /// Jump to a specific directory path inside the active tab.
    pub fn jump_to_path(&mut self, path: PathBuf) {
        let active_stats = Arc::new(Mutex::new(PathStats::default()));
        let scan_progress = Arc::new(Mutex::new(ScanProgress::default()));

        scan_path_async(
            path.clone(),
            Arc::clone(&active_stats),
            Arc::clone(&scan_progress),
        );

        let git_ctx = GitContext::read(&path);

        self.current_path = path.clone();
        self.active_stats = active_stats;
        self.scan_progress = scan_progress;
        self.navigation.update_items(Vec::new());
        self.navigation.set_cursor(0);
        self.navigation.clear_expanded();
        self.navigation.set_filter(None);
        self.all_entries = Vec::new();
        self.scan_applied = false;
        self.preview_cache = None;
        self.git_ctx = git_ctx;

        self.record_navigation(path);
        self.save_active_tab();
    }

    // Helper to copy text to system clipboard.
    fn copy_to_clipboard(&mut self, text: &str) {
        match arboard::Clipboard::new() {
            Ok(mut cb) => {
                if let Err(e) = cb.set_text(text.to_string()) {
                    self.notification =
                        Some((format!("Clipboard error: {}", e), std::time::Instant::now()));
                }
            }
            Err(e) => {
                self.notification =
                    Some((format!("Clipboard error: {}", e), std::time::Instant::now()));
            }
        }
    }

    /// Yank full path of the selected file(s) to system clipboard.
    pub fn yank_full_path(&mut self) {
        let targets = if !self.selected_paths.is_empty() {
            self.selected_paths.iter().cloned().collect::<Vec<_>>()
        } else if let Some(item) = self.selected_item() {
            vec![item.path]
        } else {
            Vec::new()
        };

        if targets.is_empty() {
            return;
        }

        let paths_str = targets
            .iter()
            .map(|p| p.to_string_lossy())
            .collect::<Vec<_>>()
            .join("\n");
        self.copy_to_clipboard(&paths_str);

        self.yanked_files = targets.clone();
        self.cut_files.clear();

        let count = targets.len();
        let name = if count == 1 {
            targets[0]
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            format!("{} items", count)
        };
        self.notification = Some((format!("Yanked: {}", name), std::time::Instant::now()));
        self.selected_paths.clear();
    }

    /// Yank filename of the selected file(s) to system clipboard.
    pub fn yank_filename(&mut self) {
        let targets = if !self.selected_paths.is_empty() {
            self.selected_paths.iter().cloned().collect::<Vec<_>>()
        } else if let Some(item) = self.selected_item() {
            vec![item.path]
        } else {
            Vec::new()
        };

        if targets.is_empty() {
            return;
        }

        let filenames = targets
            .iter()
            .map(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default()
            })
            .collect::<Vec<_>>()
            .join("\n");
        self.copy_to_clipboard(&filenames);

        let count = targets.len();
        self.notification = Some((
            format!("Yanked filename(s) of {} items", count),
            std::time::Instant::now(),
        ));
        self.selected_paths.clear();
    }

    /// Cut selected file(s) for moving later.
    pub fn cut_file(&mut self) {
        let targets = if !self.selected_paths.is_empty() {
            self.selected_paths.iter().cloned().collect::<Vec<_>>()
        } else if let Some(item) = self.selected_item() {
            vec![item.path]
        } else {
            Vec::new()
        };

        if targets.is_empty() {
            return;
        }

        self.cut_files = targets.clone();
        self.yanked_files.clear();

        let count = targets.len();
        let name = if count == 1 {
            targets[0]
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        } else {
            format!("{} items", count)
        };
        self.notification = Some((format!("Cut: {}", name), std::time::Instant::now()));
        self.selected_paths.clear();
    }

    /// Helper to recursively copy directories.
    fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            let ty = entry.file_type()?;
            let next_dst = dst.join(entry.file_name());
            if ty.is_dir() {
                Self::copy_dir_recursive(&entry.path(), &next_dst)?;
            } else {
                std::fs::copy(entry.path(), &next_dst)?;
            }
        }
        Ok(())
    }

    /// Paste yanked or cut files in the current viewing directory.
    pub fn paste_files(&mut self) {
        if !self.yanked_files.is_empty() {
            let mut errors = 0;
            let mut success = 0;
            for path in &self.yanked_files {
                if let Some(filename) = path.file_name() {
                    let dst = self.current_path.join(filename);
                    if path.is_dir() {
                        if Self::copy_dir_recursive(path, &dst).is_ok() {
                            success += 1;
                        } else {
                            errors += 1;
                        }
                    } else if std::fs::copy(path, &dst).is_ok() {
                        success += 1;
                    } else {
                        errors += 1;
                    }
                }
            }
            if errors > 0 {
                self.notification = Some((
                    format!("Pasted {} items with {} errors", success, errors),
                    std::time::Instant::now(),
                ));
            } else {
                self.notification = Some((
                    format!("Pasted {} items successfully", success),
                    std::time::Instant::now(),
                ));
            }
            // Trigger refresh
            self.jump_to_path(self.current_path.clone());
        } else if !self.cut_files.is_empty() {
            let mut errors = 0;
            let mut success = 0;
            for path in &self.cut_files {
                if let Some(filename) = path.file_name() {
                    let dst = self.current_path.join(filename);
                    if std::fs::rename(path, &dst).is_ok() {
                        success += 1;
                    } else {
                        // Fallback: copy recursively and delete source (e.g. cross-device move)
                        let copy_res = if path.is_dir() {
                            Self::copy_dir_recursive(path, &dst)
                        } else {
                            std::fs::copy(path, &dst).map(|_| ())
                        };
                        if copy_res.is_ok() {
                            let _ = if path.is_dir() {
                                std::fs::remove_dir_all(path)
                            } else {
                                std::fs::remove_file(path)
                            };
                            success += 1;
                        } else {
                            errors += 1;
                        }
                    }
                }
            }
            self.cut_files.clear();
            if errors > 0 {
                self.notification = Some((
                    format!("Moved {} items with {} errors", success, errors),
                    std::time::Instant::now(),
                ));
            } else {
                self.notification = Some((
                    format!("Moved {} items successfully", success),
                    std::time::Instant::now(),
                ));
            }
            // Trigger refresh
            self.jump_to_path(self.current_path.clone());
        }
    }

    /// Open selected file in the system default application.
    pub fn open_in_system(&mut self) {
        if let Some(item) = self.selected_item() {
            if item.entry_type == EntryType::File {
                match open::that(&item.path) {
                    Ok(_) => {
                        self.notification = Some((
                            format!(
                                "Opened: {}",
                                item.path.file_name().unwrap().to_string_lossy()
                            ),
                            std::time::Instant::now(),
                        ));
                    }
                    Err(e) => {
                        self.notification =
                            Some((format!("Open error: {}", e), std::time::Instant::now()));
                    }
                }
            }
        }
    }

    /// Request file deletion (sets up target paths and opens DeleteConfirmation modal).
    pub fn request_delete(&mut self) {
        let targets = if !self.selected_paths.is_empty() {
            self.selected_paths.iter().cloned().collect::<Vec<_>>()
        } else if let Some(item) = self.selected_item() {
            vec![item.path]
        } else {
            Vec::new()
        };

        if targets.is_empty() {
            return;
        }

        self.delete_targets = targets;
        self.modal_mode = ModalMode::DeleteConfirmation;
    }

    /// Confirm and execute file deletion.
    pub fn confirm_delete(&mut self) {
        let mut success = 0;
        let mut errors = 0;

        for path in &self.delete_targets {
            let res = if path.is_dir() {
                std::fs::remove_dir_all(path)
            } else {
                std::fs::remove_file(path)
            };

            if res.is_ok() {
                success += 1;
            } else {
                errors += 1;
            }
        }

        let count = self.delete_targets.len();
        self.delete_targets.clear();
        self.selected_paths.clear();
        self.modal_mode = ModalMode::None;

        if errors > 0 {
            self.notification = Some((
                format!("Deleted {}/{} items ({} errors)", success, count, errors),
                std::time::Instant::now(),
            ));
        } else {
            self.notification = Some((
                format!("Deleted {} items successfully", success),
                std::time::Instant::now(),
            ));
        }

        // Refresh view
        self.jump_to_path(self.current_path.clone());
    }

    /// Request file rename (initiates rename mode and sets target/buffer).
    pub fn request_rename(&mut self) {
        if let Some(item) = self.selected_item() {
            let filename = item
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            self.rename_target = Some(item.path);
            self.rename_input = filename;
            self.rename_mode = true;
            self.search_mode = false;
        }
    }

    /// Confirm and execute file rename.
    pub fn confirm_rename(&mut self) {
        if let Some(target) = self.rename_target.take() {
            if !self.rename_input.is_empty() {
                if let Some(parent) = target.parent() {
                    let dst = parent.join(&self.rename_input);
                    if std::fs::rename(&target, &dst).is_ok() {
                        let old_name = target.file_name().unwrap().to_string_lossy();
                        self.notification = Some((
                            format!("Renamed {} to {}", old_name, self.rename_input),
                            std::time::Instant::now(),
                        ));
                    } else {
                        self.notification =
                            Some(("Rename failed".to_string(), std::time::Instant::now()));
                    }
                }
            }
        }
        self.rename_mode = false;
        self.rename_input.clear();
        // Refresh view
        self.jump_to_path(self.current_path.clone());
    }

    /// Toggle selection status of the currently highlighted entry for batch actions.
    pub fn toggle_select(&mut self) {
        if let Some(item) = self.selected_item() {
            if self.selected_paths.contains(&item.path) {
                self.selected_paths.remove(&item.path);
            } else {
                self.selected_paths.insert(item.path);
            }
        }
    }

    /// Move the selection cursor by `delta` rows, wrapping at both ends.
    pub fn move_selection(&mut self, delta: isize) {
        let len = self.visible_items().len();
        if len == 0 {
            return;
        }
        let current = self.navigation.cursor();
        let idx = (current as isize + delta) % len as isize;
        let new_idx = if idx < 0 {
            (idx + len as isize) as usize
        } else {
            idx as usize
        };
        self.navigation.set_cursor(new_idx);
    }

    /// Toggle search-mode focus. Leaving search mode preserves the query so the
    /// user can inspect the filtered results before clearing it.
    pub fn toggle_search_mode(&mut self) {
        self.search_mode = !self.search_mode;
    }

    /// Clear the active query and reset the cursor to the first visible row.
    pub fn clear_search(&mut self) {
        self.navigation.set_filter(None);
    }

    /// Append one typed character to the live query.
    pub fn push_search_char(&mut self, ch: char) {
        let mut query = self.navigation.filter_query().unwrap_or("").to_string();
        query.push(ch);
        self.navigation.set_filter(Some(query));
    }

    /// Delete the most recent search character.
    pub fn pop_search_char(&mut self) {
        let mut query = self.navigation.filter_query().unwrap_or("").to_string();
        query.pop();
        if query.is_empty() {
            self.navigation.set_filter(None);
        } else {
            self.navigation.set_filter(Some(query));
        }
    }

    /// Fuzzy match query over files and directories using the Nucleo matcher engine.
    pub fn filter_query(&self, query: &str) -> Vec<(DirEntry, u32)> {
        if query.is_empty() {
            return self
                .all_entries
                .iter()
                .map(|entry| (entry.clone(), 0))
                .collect();
        }

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(query, CaseMatching::Ignore, Normalization::Smart);

        struct MatchItem<'a> {
            entry: &'a DirEntry,
        }

        impl<'a> AsRef<str> for MatchItem<'a> {
            fn as_ref(&self) -> &str {
                &self.entry.display_path
            }
        }

        let items: Vec<MatchItem> = self
            .all_entries
            .iter()
            .map(|entry| MatchItem { entry })
            .collect();

        let mut matches = pattern.match_list(items, &mut matcher);

        // Rank results by score descending, then by path alphabetically for stability
        matches.sort_by(|a, b| {
            b.1.cmp(&a.1)
                .then_with(|| a.0.entry.path.cmp(&b.0.entry.path))
        });

        matches
            .into_iter()
            .map(|(item, score)| (item.entry.clone(), score))
            .collect()
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
    fn test_move_selection_arbitrary_delta() {
        let (mut state, _dir) = make_state_with_subdirs(); // Has 3 items (indices 0, 1, 2)
        state.selected_index = 1;
        state.move_selection(5); // 1 + 5 = 6 -> 6 % 3 = 0
        assert_eq!(state.selected_index, 0);

        state.selected_index = 1;
        state.move_selection(-5); // 1 - 5 = -4 -> -4 % 3 = -1 -> -1 + 3 = 2
        assert_eq!(state.selected_index, 2);
    }

    #[test]
    fn test_navigate_out_does_not_panic() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());
        // Ascending from a temp dir must not panic regardless of depth.
        state.navigate_out();
    }

    fn mock_dir_entry(path: PathBuf, size: u64, is_dir: bool) -> DirEntry {
        DirEntry {
            path,
            size,
            entry_type: if is_dir {
                EntryType::Directory
            } else {
                EntryType::File
            },
            mtime: std::time::SystemTime::UNIX_EPOCH,
            display_path: String::new(),
            symlink_target: None,
        }
    }

    #[test]
    fn test_live_filter_items() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let cargo_path = dir.path().join("Cargo.toml");
        let main_path = src_dir.join("main.rs");
        let app_path = src_dir.join("app.rs");

        File::create(&cargo_path).unwrap();
        File::create(&main_path).unwrap();
        File::create(&app_path).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        let filtered = state.filter_query("main");
        assert_eq!(filtered.len(), 1);
        assert!(filtered[0].0.path.ends_with(PathBuf::from("src/main.rs")));
    }

    #[test]
    fn test_visible_items_uses_search_query() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src");
        std::fs::create_dir_all(&src_dir).unwrap();

        let cargo_path = dir.path().join("Cargo.toml");
        let main_path = src_dir.join("main.rs");
        let app_path = src_dir.join("app.rs");

        File::create(&cargo_path).unwrap();
        File::create(&main_path).unwrap();
        File::create(&app_path).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);
        state.search_query = String::from("app");

        let visible = state.visible_items();
        assert_eq!(visible.len(), 1);
        assert!(visible[0].0.path.ends_with(PathBuf::from("src/app.rs")));
    }

    #[test]
    fn test_search_editing_resets_selection() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);
        state.items = vec![
            mock_dir_entry(PathBuf::from("alpha"), 1, true),
            mock_dir_entry(PathBuf::from("beta"), 1, true),
        ];
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

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);
        let filtered = state.filter_query("main.rs");

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].0.path, file_path);
    }

    #[test]
    fn test_is_scanning_and_poll_scan() {
        let dir = tempdir().unwrap();
        let mut f = File::create(dir.path().join("x.txt")).unwrap();
        f.write_all(b"data").unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
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
        let mut f = File::create(dir.path().join("only_file.txt")).unwrap();
        f.write_all(b"data").unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        assert!(!state.is_scanning());
        assert_eq!(state.items.len(), 1);
        assert!(state.scan_applied);

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

        state.items = vec![mock_dir_entry(file_path.clone(), 12, false)];
        state.selected_index = 0;

        assert!(state.preview_lines().is_empty());

        state.update_preview_cache();

        let lines = state.preview_lines();
        assert!(!lines.is_empty());
        assert!(lines[0].to_string().contains("fn test()"));

        state.items.clear();
        state.update_preview_cache();
        assert!(state.preview_lines().is_empty());
    }

    #[test]
    fn test_fuzzy_matches_transpositions() {
        let dir = tempdir().unwrap();
        let main_path = dir.path().join("main.rs");
        let cargo_path = dir.path().join("Cargo.toml");
        File::create(&main_path).unwrap();
        File::create(&cargo_path).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        let matches_mrs = state.filter_query("mrs");
        assert_eq!(matches_mrs.len(), 1);
        assert_eq!(matches_mrs[0].0.path, main_path);

        let matches_crgo = state.filter_query("crgo");
        assert_eq!(matches_crgo.len(), 1);
        assert_eq!(matches_crgo[0].0.path, cargo_path);
    }

    #[test]
    fn test_empty_query_returns_all() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("a.rs");
        let f2 = dir.path().join("b.rs");
        File::create(&f1).unwrap();
        File::create(&f2).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        let filtered = state.filter_query("");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_no_match_returns_empty() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("a.rs");
        File::create(&f1).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        let filtered = state.filter_query("nonexistent");
        assert!(filtered.is_empty());
    }

    #[test]
    fn test_sorting_modes() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());

        let t1 = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100);
        let t2 = std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200);

        let e1 = DirEntry {
            path: PathBuf::from("z_file"),
            size: 50,
            entry_type: EntryType::Directory,
            mtime: t1,
            display_path: String::new(),
            symlink_target: None,
        };
        let e2 = DirEntry {
            path: PathBuf::from("a_file"),
            size: 100,
            entry_type: EntryType::Directory,
            mtime: t2,
            display_path: String::new(),
            symlink_target: None,
        };

        state.items = vec![e1.clone(), e2.clone()];

        state.sort_items();
        assert_eq!(state.items[0].path, PathBuf::from("a_file"));

        state.sort_mode = SortMode::NameAsc;
        state.sort_items();
        assert_eq!(state.items[0].path, PathBuf::from("a_file"));

        state.sort_mode = SortMode::MtimeDesc;
        state.sort_items();
        assert_eq!(state.items[0].path, PathBuf::from("a_file"));

        state.sort_mode = SortMode::SizeDesc;
        state.cycle_sort_mode();
        assert_eq!(state.sort_mode, SortMode::NameAsc);
    }

    #[test]
    fn test_directory_expansion() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("sub_dir");
        std::fs::create_dir(&sub).unwrap();
        let child_file = sub.join("child.txt");
        {
            let mut f = File::create(&child_file).unwrap();
            f.write_all(b"hello world").unwrap();
        }

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        // Before expansion: only directory is visible (top-level items only contains directories)
        let visible_before = state.visible_items();
        assert_eq!(visible_before.len(), 1);
        assert_eq!(visible_before[0].0.path, sub);

        // Select the directory and toggle expand
        state.selected_index = 0;
        state.toggle_expand();

        // After expansion: both directory and child_file should be visible
        let visible_after = state.visible_items();
        assert_eq!(visible_after.len(), 2);
        assert_eq!(visible_after[0].0.path, sub);
        assert_eq!(visible_after[1].0.path, child_file);

        // Toggle expand again to collapse
        state.toggle_expand();

        // After collapse: only directory is visible again
        let visible_collapsed = state.visible_items();
        assert_eq!(visible_collapsed.len(), 1);
        assert_eq!(visible_collapsed[0].0.path, sub);
    }

    #[test]
    fn test_tab_management() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        // Initial state has 1 tab
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab, 0);

        // Try to close the last tab -> should be rejected (len stays 1)
        state.close_tab();
        assert_eq!(state.tabs.len(), 1);

        // Open a new tab at the same path
        state.open_tab(dir.path().to_path_buf());
        assert_eq!(state.tabs.len(), 2);
        assert_eq!(state.active_tab, 1);

        // Switch back to the first tab using prev_tab
        state.prev_tab();
        assert_eq!(state.active_tab, 0);

        // Switch to the second tab using next_tab
        state.next_tab();
        assert_eq!(state.active_tab, 1);

        // Close the second tab -> active tab should fall back to 0
        state.close_tab();
        assert_eq!(state.tabs.len(), 1);
        assert_eq!(state.active_tab, 0);
    }

    #[test]
    fn test_bookmarks_and_recent_history() {
        let dir = tempdir().unwrap();
        let config_file = dir.path().join("bookmarks.json");
        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = Some(config_file.clone()));

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        // Active path should be automatically added to recent history on startup
        assert_eq!(state.config.recent.len(), 1);
        assert_eq!(state.config.recent[0], dir.path());

        // Add a bookmark
        state.add_bookmark();
        assert_eq!(state.config.bookmarks.len(), 1);
        assert_eq!(state.config.bookmarks[0], dir.path());

        // Try adding duplicates (should not duplicate)
        state.add_bookmark();
        assert_eq!(state.config.bookmarks.len(), 1);

        // Remove bookmark
        state.remove_bookmark(0);
        assert_eq!(state.config.bookmarks.len(), 0);

        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = None);
    }

    #[test]
    fn test_config_persistence_and_deduplication() {
        let dir = tempdir().unwrap();
        let config_file = dir.path().join("bookmarks.json");
        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = Some(config_file.clone()));

        let mut config = crate::config::Config::default();
        config.bookmarks.push(PathBuf::from("/a"));
        config.bookmarks.push(PathBuf::from("/a")); // duplicate
        config.bookmarks.push(PathBuf::from("/b"));
        config.save();

        // Load config from file
        let loaded = crate::config::Config::load();
        // Duplicate "/a" should be stripped/deduplicated on load
        assert_eq!(loaded.bookmarks.len(), 2);
        assert_eq!(loaded.bookmarks[0], PathBuf::from("/a"));
        assert_eq!(loaded.bookmarks[1], PathBuf::from("/b"));

        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = None);
    }

    #[test]
    fn test_remove_recent() {
        let dir = tempdir().unwrap();
        let config_file = dir.path().join("bookmarks.json");
        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = Some(config_file.clone()));

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        state.record_navigation(PathBuf::from("/recent_a"));
        state.record_navigation(PathBuf::from("/recent_b"));

        assert!(state.config.recent.contains(&PathBuf::from("/recent_a")));
        assert!(state.config.recent.contains(&PathBuf::from("/recent_b")));

        // Let's find index of /recent_a
        let idx = state
            .config
            .recent
            .iter()
            .position(|p| p == &PathBuf::from("/recent_a"))
            .unwrap();
        state.remove_recent(idx);

        assert!(!state.config.recent.contains(&PathBuf::from("/recent_a")));

        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = None);
    }

    #[test]
    fn test_bookmark_notification() {
        let dir = tempdir().unwrap();
        let config_file = dir.path().join("bookmarks.json");
        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = Some(config_file.clone()));

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        assert!(state.notification.is_none());
        state.add_bookmark();
        assert!(state.notification.is_some());
        let (msg, _) = state.notification.unwrap();
        assert!(msg.contains("Added to Bookmarks:"));

        crate::config::TEST_CONFIG_PATH.with(|p| *p.borrow_mut() = None);
    }

    #[test]
    fn test_toggle_select_files() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("file1.txt");
        File::create(&f1).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        state.items = vec![mock_dir_entry(f1.clone(), 0, false)];
        state.selected_index = 0;

        assert!(state.selected_paths.is_empty());
        state.toggle_select();
        assert_eq!(state.selected_paths.len(), 1);
        assert!(state.selected_paths.contains(&f1));

        state.toggle_select();
        assert!(state.selected_paths.is_empty());
    }

    #[test]
    fn test_yank_and_paste_file() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src_folder");
        let dst_dir = dir.path().join("dst_folder");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::create_dir(&dst_dir).unwrap();

        let file_path = src_dir.join("test.txt");
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(b"yank content").unwrap();
        }

        let mut state = AppState::new(dst_dir.clone());
        wait_for_scan(&mut state);

        // Mock current item
        state.items = vec![mock_dir_entry(file_path.clone(), 12, false)];
        state.selected_index = 0;

        state.yank_full_path();
        assert_eq!(state.yanked_files.len(), 1);
        assert_eq!(state.yanked_files[0], file_path);

        // Paste into state.current_path (which is dst_dir)
        state.paste_files();

        let pasted_path = dst_dir.join("test.txt");
        assert!(pasted_path.exists());
        assert_eq!(
            std::fs::read_to_string(pasted_path).unwrap(),
            "yank content"
        );
    }

    #[test]
    fn test_cut_and_paste_file() {
        let dir = tempdir().unwrap();
        let src_dir = dir.path().join("src_folder");
        let dst_dir = dir.path().join("dst_folder");
        std::fs::create_dir(&src_dir).unwrap();
        std::fs::create_dir(&dst_dir).unwrap();

        let file_path = src_dir.join("test.txt");
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(b"cut content").unwrap();
        }

        let mut state = AppState::new(dst_dir.clone());
        wait_for_scan(&mut state);

        state.items = vec![mock_dir_entry(file_path.clone(), 11, false)];
        state.selected_index = 0;

        state.cut_file();
        assert_eq!(state.cut_files.len(), 1);
        assert_eq!(state.cut_files[0], file_path);

        state.paste_files();

        let pasted_path = dst_dir.join("test.txt");
        assert!(pasted_path.exists());
        assert!(!file_path.exists());
        assert_eq!(std::fs::read_to_string(pasted_path).unwrap(), "cut content");
    }

    #[test]
    fn test_rename_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("old_name.txt");
        {
            let mut f = File::create(&file_path).unwrap();
            f.write_all(b"rename content").unwrap();
        }

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        state.items = vec![mock_dir_entry(file_path.clone(), 14, false)];
        state.selected_index = 0;

        state.request_rename();
        assert!(state.rename_mode);
        assert_eq!(state.rename_input, "old_name.txt");
        assert_eq!(state.rename_target, Some(file_path.clone()));

        state.rename_input = "new_name.txt".to_string();
        state.confirm_rename();

        assert!(!state.rename_mode);
        assert!(state.rename_input.is_empty());

        let new_file_path = dir.path().join("new_name.txt");
        assert!(new_file_path.exists());
        assert!(!file_path.exists());
    }

    #[test]
    fn test_delete_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("delete_me.txt");
        File::create(&file_path).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        state.items = vec![mock_dir_entry(file_path.clone(), 0, false)];
        state.selected_index = 0;

        state.request_delete();
        assert_eq!(state.modal_mode, ModalMode::DeleteConfirmation);
        assert_eq!(state.delete_targets.len(), 1);
        assert_eq!(state.delete_targets[0], file_path);

        state.confirm_delete();
        assert_eq!(state.modal_mode, ModalMode::None);
        assert!(state.delete_targets.is_empty());
        assert!(!file_path.exists());
    }

    #[test]
    fn test_help_modal_state() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());
        assert!(!state.show_help);
        assert_eq!(state.help_selected_index, 0);

        state.show_help = true;
        assert!(state.show_help);
    }
}
