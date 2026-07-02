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

use crate::fs::walker::{scan_immediate, PathStats, ScanProgress};
use crate::git::GitContext;
use image::GenericImageView as _;
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

#[derive(Debug, Clone)]
pub struct PluginOverlayItem {
    pub label: String,
    pub value: String,
}

pub struct ShellResult {
    pub callback_key: mlua::RegistryKey,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModalMode {
    None,
    Bookmarks,
    Recent,
    OpenConfirmation,
    DeleteConfirmation,
    SearchOverlay,
    CommandPalette,
    SizeDetails,
    PluginOverlay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOverlayMode {
    FuzzyFiles,
    LiveGrep,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    pub path: PathBuf,
    pub line_number: Option<usize>,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewType {
    Text,
    Image,
    Unsupported,
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
    pub preview_cache: Option<(PathBuf, String, Option<usize>, Vec<Line<'static>>)>,
    pub git_ctx: Option<GitContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRenderCache {
    pub path: PathBuf,
    pub area: ratatui::layout::Rect,
    pub sequence: String,
    pub protocol: crate::preview::TerminalProtocol,
}

#[derive(Debug)]
pub struct PreviewTask {
    pub path: PathBuf,
    pub query: String,
    pub width: u16,
    pub height: u16,
    pub protocol: crate::preview::TerminalProtocol,
}

/// Central state passed to every render call and mutated by keyboard events.
pub struct AppState {
    pub current_path: PathBuf,
    pub active_stats: Arc<Mutex<PathStats>>,
    pub scan_progress: Arc<Mutex<ScanProgress>>,
    pub navigation: crate::navigation::Navigation,
    pub all_entries: Vec<DirEntry>,
    scan_applied: bool,
    preview_cache: Option<(PathBuf, String, Option<usize>, Vec<Line<'static>>)>,
    pub last_rendered_image: std::sync::Mutex<Option<ImageRenderCache>>,
    pub preview_tx: std::sync::mpsc::Sender<(
        PathBuf,
        String,
        Vec<Line<'static>>,
        Option<ImageRenderCache>,
    )>,
    preview_rx: std::sync::mpsc::Receiver<(
        PathBuf,
        String,
        Vec<Line<'static>>,
        Option<ImageRenderCache>,
    )>,
    pub worker_tx: std::sync::mpsc::Sender<PreviewTask>,
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
    pub plugin_engine: Option<crate::plugin::engine::PluginEngine>,
    pub last_selection_time: std::time::Instant,
    pub search_overlay_mode: SearchOverlayMode,
    pub search_overlay_input: String,
    pub search_overlay_results: Vec<SearchMatch>,
    pub search_overlay_selected_index: usize,
    pub search_overlay_cursor_index: usize,
    pub search_overlay_focused: bool,
    pub rg_query_tx: std::sync::mpsc::Sender<crate::search::ripgrep::RgSearchQuery>,
    pub rg_match_rx: std::sync::mpsc::Receiver<crate::search::ripgrep::RgMessage>,
    pub command_palette_input: String,
    pub command_palette_candidates: Vec<crate::project::detector::DetectedCommand>,
    pub command_palette_results: Vec<crate::project::detector::DetectedCommand>,
    pub command_palette_selected_index: usize,
    pub command_palette_cursor_index: usize,
    pub command_palette_focused: bool,
    pub size_popup_path: Option<PathBuf>,
    pub size_popup_stats: Option<Arc<Mutex<PathStats>>>,
    pub size_popup_progress: Option<Arc<Mutex<ScanProgress>>>,
    pub right_pane_dashboard_cache: std::cell::RefCell<Option<(PathBuf, FolderDashboardSummary)>>,
    pub plugin_modal_title: String,
    pub plugin_modal_items: Vec<PluginOverlayItem>,
    pub plugin_modal_filtered_items: Vec<PluginOverlayItem>,
    pub plugin_modal_input: String,
    pub plugin_modal_selected_index: usize,
    pub shell_result_tx: std::sync::mpsc::Sender<ShellResult>,
    pub shell_result_rx: std::sync::mpsc::Receiver<ShellResult>,
}

#[derive(Debug, Clone)]
pub struct FolderDashboardSummary {
    pub path: PathBuf,
    pub file_count: usize,
    pub dir_count: usize,
    pub total_immediate_size: u64,
    pub top_files: Vec<(String, u64)>,          // Name and size
    pub extension_counts: Vec<(String, usize)>, // Extension and count
}

impl FolderDashboardSummary {
    pub fn calculate(path: &std::path::Path) -> Self {
        let mut file_count = 0;
        let mut dir_count = 0;
        let mut total_immediate_size = 0;
        let mut files = Vec::new();
        let mut ext_counts: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        if let Ok(read_dir) = std::fs::read_dir(path) {
            for entry in read_dir.flatten() {
                let file_type = entry.file_type();
                let name = entry.file_name().to_string_lossy().to_string();
                if let Ok(ft) = file_type {
                    if ft.is_dir() {
                        dir_count += 1;
                    } else if ft.is_file() {
                        file_count += 1;
                        let metadata = entry.metadata();
                        let size = metadata.map(|m| m.len()).unwrap_or(0);
                        total_immediate_size += size;
                        files.push((name.clone(), size));

                        // Get extension
                        let ext = std::path::Path::new(&name)
                            .extension()
                            .map(|e| e.to_string_lossy().to_string().to_lowercase())
                            .unwrap_or_else(|| "no ext".to_string());
                        *ext_counts.entry(ext).or_insert(0) += 1;
                    }
                }
            }
        }

        // Sort files by size descending, keep top 5
        files.sort_by_key(|x| std::cmp::Reverse(x.1));
        files.truncate(5);

        // Sort extension counts by frequency descending
        let mut extension_counts: Vec<(String, usize)> = ext_counts.into_iter().collect();
        extension_counts.sort_by_key(|x| std::cmp::Reverse(x.1));

        Self {
            path: path.to_path_buf(),
            file_count,
            dir_count,
            total_immediate_size,
            top_files: files,
            extension_counts,
        }
    }
}

// --------------------------------------------------------------------------
// [SECTION] State Machine
// --------------------------------------------------------------------------

impl AppState {
    #[allow(clippy::type_complexity)]
    fn prepare_directory_load(
        path: &std::path::Path,
    ) -> (
        Arc<Mutex<PathStats>>,
        Arc<Mutex<ScanProgress>>,
        Option<GitContext>,
        Vec<DirEntry>,
    ) {
        let active_stats = Arc::new(Mutex::new(PathStats::default()));
        let scan_progress = Arc::new(Mutex::new(ScanProgress::Complete));

        let git_ctx = GitContext::read(path);
        let immediate_entries = scan_immediate(path).unwrap_or_default();
        (active_stats, scan_progress, git_ctx, immediate_entries)
    }

    /// Create a new AppState pointing to `root` and start scanning. The TUI will
    /// render immediately with a progress spinner while the scan proceeds until
    /// the background thread finishes.
    pub fn new(root: PathBuf) -> Self {
        let (active_stats, scan_progress, git_ctx, immediate_entries) =
            Self::prepare_directory_load(&root);

        let mut config = crate::config::Config::load();
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
            navigation: crate::navigation::Navigation::new(
                immediate_entries.clone(),
                SortMode::SizeDesc,
            ),
            all_entries: immediate_entries.clone(),
            scan_applied: false,
            preview_cache: None,
            git_ctx: git_ctx.clone(),
        };

        let (preview_tx, preview_rx) = std::sync::mpsc::channel();
        let (worker_tx, worker_rx) = std::sync::mpsc::channel::<PreviewTask>();

        let (rg_query_tx, rg_query_rx) = std::sync::mpsc::channel();
        let (rg_match_tx, rg_match_rx) = std::sync::mpsc::channel();
        let (shell_result_tx, shell_result_rx) = std::sync::mpsc::channel();

        std::thread::spawn(move || {
            crate::search::ripgrep::spawn_rg_worker(rg_query_rx, rg_match_tx);
        });

        let tx_clone = preview_tx.clone();
        std::thread::spawn(move || {
            while let Ok(mut task) = worker_rx.recv() {
                // Drain any newer tasks in the channel to keep only the absolute latest
                while let Ok(newer) = worker_rx.try_recv() {
                    task = newer;
                }

                // Process the task
                let is_pdf = task
                    .path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|s| s.to_lowercase())
                    == Some("pdf".to_string());
                let target_path = if is_pdf {
                    match crate::preview::extract_pdf_first_page(&task.path) {
                        Ok(p) => p,
                        Err(err) => {
                            let lines = vec![Line::from(format!("[x] PDF preview error: {err}"))];
                            let _ = tx_clone.send((task.path, task.query, lines, None));
                            continue;
                        }
                    }
                } else {
                    task.path.clone()
                };

                match task.protocol {
                    crate::preview::TerminalProtocol::HalfBlock => {
                        if *crate::preview::HAS_CHAFA {
                            if let Ok(lines) = crate::preview::render_chafa_symbols(
                                &target_path,
                                task.width,
                                task.height,
                            ) {
                                let _ = tx_clone.send((task.path, task.query, lines, None));
                                continue;
                            }
                        }
                        if let Ok(img) = image::open(&target_path) {
                            let lines =
                                crate::preview::render_half_block(&img, task.width, task.height);
                            let _ = tx_clone.send((task.path, task.query, lines, None));
                        } else {
                            let lines = vec![Line::from("[Error loading image]")];
                            let _ = tx_clone.send((task.path, task.query, lines, None));
                        }
                    }
                    crate::preview::TerminalProtocol::Kitty => {
                        let display_h = task.height.saturating_sub(1);
                        if let Ok(img) = image::open(&target_path) {
                            let resized =
                                img.thumbnail(task.width as u32 * 12, display_h as u32 * 24);
                            let mut png_bytes = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut png_bytes);
                            if resized
                                .write_to(&mut cursor, image::ImageFormat::Png)
                                .is_ok()
                            {
                                let (w, h) = resized.dimensions();
                                let cols = (w / 12).max(1u32) as u16;
                                let rows = (h / 24).max(1u32) as u16;
                                let sequence =
                                    crate::preview::build_kitty_sequence(&png_bytes, cols, rows);
                                let cache = ImageRenderCache {
                                    path: task.path.clone(),
                                    area: ratatui::layout::Rect::new(0, 0, task.width, display_h),
                                    sequence,
                                    protocol: task.protocol,
                                };
                                let _ = tx_clone.send((task.path, task.query, vec![], Some(cache)));
                                continue;
                            }
                        }
                        let lines = vec![Line::from("[Error loading image]")];
                        let _ = tx_clone.send((task.path, task.query, lines, None));
                    }
                    crate::preview::TerminalProtocol::Iterm2 => {
                        let display_h = task.height.saturating_sub(1);
                        if let Ok(img) = image::open(&target_path) {
                            let resized =
                                img.thumbnail(task.width as u32 * 16, display_h as u32 * 32);
                            let mut jpeg_bytes = Vec::new();
                            let mut cursor = std::io::Cursor::new(&mut jpeg_bytes);
                            if resized
                                .write_to(&mut cursor, image::ImageFormat::Jpeg)
                                .is_ok()
                            {
                                let (w, h) = resized.dimensions();
                                let cols = (w / 16).max(1u32) as u16;
                                let rows = (h / 32).max(1u32) as u16;
                                let sequence =
                                    crate::preview::build_iterm2_sequence(&jpeg_bytes, cols, rows);
                                let cache = ImageRenderCache {
                                    path: task.path.clone(),
                                    area: ratatui::layout::Rect::new(0, 0, task.width, display_h),
                                    sequence,
                                    protocol: task.protocol,
                                };
                                let _ = tx_clone.send((task.path, task.query, vec![], Some(cache)));
                                continue;
                            }
                        }
                        let lines = vec![Line::from("[Error loading image]")];
                        let _ = tx_clone.send((task.path, task.query, lines, None));
                    }
                    crate::preview::TerminalProtocol::Sixel => {
                        let display_h = task.height.saturating_sub(1);
                        if *crate::preview::HAS_CHAFA {
                            if let Ok(sequence) = crate::preview::build_sixel_sequence_via_chafa(
                                &target_path,
                                task.width,
                                display_h,
                            ) {
                                let cache = ImageRenderCache {
                                    path: task.path.clone(),
                                    area: ratatui::layout::Rect::new(0, 0, task.width, display_h),
                                    sequence,
                                    protocol: task.protocol,
                                };
                                let _ = tx_clone.send((task.path, task.query, vec![], Some(cache)));
                                continue;
                            }
                        }
                        if let Ok(img) = image::open(&target_path) {
                            let lines =
                                crate::preview::render_half_block(&img, task.width, task.height);
                            let _ = tx_clone.send((task.path, task.query, lines, None));
                        } else {
                            let lines = vec![Line::from("[Error loading image]")];
                            let _ = tx_clone.send((task.path, task.query, lines, None));
                        }
                    }
                }
            }
        });

        let mut state = Self {
            current_path: root.clone(),
            active_stats,
            scan_progress,
            navigation: crate::navigation::Navigation::new(
                immediate_entries.clone(),
                SortMode::SizeDesc,
            ),
            all_entries: immediate_entries,
            scan_applied: false,
            preview_cache: None,
            last_rendered_image: std::sync::Mutex::new(None),
            preview_tx,
            preview_rx,
            worker_tx,
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
            plugin_engine: None,
            last_selection_time: std::time::Instant::now(),
            search_overlay_mode: SearchOverlayMode::FuzzyFiles,
            search_overlay_input: String::new(),
            search_overlay_results: Vec::new(),
            search_overlay_selected_index: 0,
            search_overlay_cursor_index: 0,
            search_overlay_focused: true,
            rg_query_tx,
            rg_match_rx,
            command_palette_input: String::new(),
            command_palette_candidates: Vec::new(),
            command_palette_results: Vec::new(),
            command_palette_selected_index: 0,
            command_palette_cursor_index: 0,
            command_palette_focused: true,
            size_popup_path: None,
            size_popup_stats: None,
            size_popup_progress: None,
            right_pane_dashboard_cache: std::cell::RefCell::new(None),
            plugin_modal_title: String::new(),
            plugin_modal_items: Vec::new(),
            plugin_modal_filtered_items: Vec::new(),
            plugin_modal_input: String::new(),
            plugin_modal_selected_index: 0,
            shell_result_tx,
            shell_result_rx,
        };

        // Discovered project commands
        let mut candidates = crate::project::detector::detect_project_commands(&root);
        // Discovered user session commands
        let session_cfg = crate::config::session::parse_session_config(&root);
        for custom in session_cfg.commands {
            candidates.push(crate::project::detector::DetectedCommand {
                name: custom.name,
                cmd: custom.cmd,
                source: ".ascope.toml".to_string(),
            });
        }
        // System commands
        candidates.push(crate::project::detector::DetectedCommand {
            name: "Reload Plugins".to_string(),
            cmd: "reload_plugins".to_string(),
            source: "System".to_string(),
        });

        state.command_palette_candidates = candidates.clone();
        state.command_palette_results = candidates;

        // Set the thread-local state pointer during plugin loading
        crate::plugin::engine::set_current_app_state(&mut state as *mut Self);
        let mut plugin_engine =
            crate::plugin::engine::PluginEngine::new(root.join(".config/ascope/plugins")).ok();
        if let Some(ref mut engine) = plugin_engine {
            let _ = engine.load_plugins();
            let _ = engine.trigger_event("on_startup", String::new());
        }
        state.plugin_engine = plugin_engine;

        state
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
            if !stats.all_entries.is_empty() {
                let new_items: Vec<DirEntry> = stats
                    .all_entries
                    .iter()
                    .filter(|entry| entry.path.parent() == Some(&self.current_path))
                    .cloned()
                    .collect();
                self.all_entries = stats.all_entries.clone();
                drop(stats);

                // Save currently selected item's path
                let selected_path = self.navigation.current_selection().map(|e| e.path.clone());

                // Use Navigation to update items (handles sorting automatically)
                self.navigation.update_items(new_items);
                self.sync_navigation_items();

                // Restore selection
                if let Some(path) = selected_path {
                    self.navigation.select_path(&path);
                }

                if let Some(query) = self.navigation.filter_query().map(String::from) {
                    self.navigation.set_filter(Some(query), &self.all_entries);
                }
            }
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
        self.sync_navigation_items();
        self.reset_selection_timeout();
    }

    pub fn detect_preview_type(&self, path: &std::path::Path) -> PreviewType {
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            match ext.to_lowercase().as_str() {
                "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "ico" | "pdf" => {
                    PreviewType::Image
                }
                "zip" | "tar" | "gz" | "7z" | "rar" | "bin" | "exe" | "dll" | "so" | "dylib"
                | "dmg" | "iso" | "docx" | "xlsx" | "pptx" => PreviewType::Unsupported,
                _ => PreviewType::Text,
            }
        } else {
            PreviewType::Text
        }
    }

    /// Check if the currently highlighted item is a file and update the
    /// preview cache if the selected file or search query has changed.
    /// Poll and apply asynchronous image/PDF preview updates.
    pub fn poll_preview_updates(&mut self) {
        while let Ok((path, query, lines, cache)) = self.preview_rx.try_recv() {
            let selected = self.selected_item().map(|x| x.path);
            let current_query = self.navigation.filter_query().unwrap_or("").to_string();

            // Only apply the preview if it matches the currently highlighted item and search query
            if Some(&path) == selected.as_ref() && current_query == query {
                self.preview_cache = Some((path, query, None, lines));
                *self.last_rendered_image.lock().unwrap() = cache;
            }
        }
    }

    pub fn poll_shell_updates(&mut self) {
        while let Ok(result) = self.shell_result_rx.try_recv() {
            if let Some(ref mut engine) = self.plugin_engine {
                let _ = engine.execute_shell_callback(
                    result.callback_key,
                    result.stdout,
                    result.stderr,
                    result.exit_code,
                );
            }
        }
    }

    pub fn poll_search_updates(&mut self) {
        while let Ok(msg) = self.rg_match_rx.try_recv() {
            if self.modal_mode == ModalMode::SearchOverlay
                && self.search_overlay_mode == SearchOverlayMode::LiveGrep
            {
                match msg {
                    crate::search::ripgrep::RgMessage::Match(m) => {
                        if self.search_overlay_results.len() < 200 {
                            let text = format!(
                                "{}:{}: {}",
                                m.path.file_name().unwrap_or_default().to_string_lossy(),
                                m.line_number,
                                m.text.trim_end()
                            );
                            self.search_overlay_results.push(SearchMatch {
                                path: m.path,
                                line_number: Some(m.line_number),
                                text,
                            });
                        }
                    }
                    crate::search::ripgrep::RgMessage::Finished => {}
                }
            }
        }
    }

    pub fn update_preview_cache(&mut self, width: u16, height: u16) {
        // Poll for asynchronous preview results first
        self.poll_preview_updates();
        // Poll for search updates
        self.poll_search_updates();

        let (selected, target_line) = if self.modal_mode == ModalMode::SearchOverlay {
            if let Some(res) = self
                .search_overlay_results
                .get(self.search_overlay_selected_index)
            {
                (Some(res.path.clone()), res.line_number)
            } else {
                (None, None)
            }
        } else {
            (self.selected_item().map(|x| x.path), None)
        };

        let query = if self.modal_mode == ModalMode::SearchOverlay {
            self.search_overlay_input.clone()
        } else {
            self.navigation.filter_query().unwrap_or("").to_string()
        };

        if let Some((cached_path, cached_query, cached_target_line, _)) = &self.preview_cache {
            if Some(cached_path) == selected.as_ref()
                && cached_query == &query
                && *cached_target_line == target_line
            {
                return;
            }
        }

        if let Some(path) = selected {
            if path.is_file() {
                let p_type = self.detect_preview_type(&path);
                if p_type == PreviewType::Image {
                    // Start loading in the background thread asynchronously
                    self.preview_cache = Some((
                        path.clone(),
                        query.clone(),
                        None,
                        vec![Line::from("[Loading preview...]")],
                    ));
                    *self.last_rendered_image.lock().unwrap() = None;

                    let protocol = crate::preview::detect_protocol();
                    let task = PreviewTask {
                        path: path.clone(),
                        query: query.clone(),
                        width,
                        height,
                        protocol,
                    };
                    let _ = self.worker_tx.send(task);
                    return;
                }

                let lines =
                    crate::ui::widgets::build_preview_lines_focused(&path, &query, target_line);
                self.preview_cache = Some((path, query, target_line, lines));
                *self.last_rendered_image.lock().unwrap() = None;
                return;
            }
        }

        self.preview_cache = None;
        *self.last_rendered_image.lock().unwrap() = None;
    }

    /// Access the cached highlighted lines of the currently selected file.
    pub fn preview_lines(&self) -> &[Line<'static>] {
        if let Some((_, _, _, lines)) = &self.preview_cache {
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
        if let Some(entry) = self.navigation.current_selection().cloned() {
            if entry.entry_type == EntryType::Directory {
                let path = entry.path.clone();
                self.navigation.toggle_expand_selected();

                // If it is now expanded, ensure its immediate children are loaded dynamically
                if self.navigation.is_expanded(&path) {
                    let has_children = self
                        .all_entries
                        .iter()
                        .any(|e| e.path.parent() == Some(&path));
                    if !has_children {
                        if let Ok(children) = scan_immediate(&path) {
                            self.all_entries.extend(children);
                        }
                    }
                }
            }
        }
        self.sync_navigation_items();
        self.reset_selection_timeout();
    }

    /// Trigger the size details popup for the currently selected directory.
    pub fn trigger_size_details_popup(&mut self) {
        if let Some(entry) = self.selected_item() {
            if entry.entry_type == EntryType::Directory {
                let path = entry.path.clone();
                self.modal_mode = ModalMode::SizeDetails;
                self.size_popup_path = Some(path.clone());

                let stats = Arc::new(Mutex::new(PathStats::default()));
                let progress = Arc::new(Mutex::new(ScanProgress::Idle));

                self.size_popup_stats = Some(stats.clone());
                self.size_popup_progress = Some(progress.clone());

                crate::fs::walker::scan_path_async(path, stats, progress);
            } else {
                self.notification = Some((
                    "Cannot scan size: selected item is a file (must select a folder)".to_string(),
                    std::time::Instant::now(),
                ));
            }
        } else {
            self.notification = Some((
                "No item selected to scan".to_string(),
                std::time::Instant::now(),
            ));
        }
    }

    /// Close the size details popup and clear the popup state.
    pub fn close_size_details_popup(&mut self) {
        self.modal_mode = ModalMode::None;
        self.size_popup_path = None;
        self.size_popup_stats = None;
        self.size_popup_progress = None;
    }

    /// Retrieve the dashboard summary for the selected directory, using cache if available.
    pub fn get_folder_dashboard(&self, path: &std::path::Path) -> FolderDashboardSummary {
        if let Some((ref cached_path, ref summary)) = *self.right_pane_dashboard_cache.borrow() {
            if cached_path == path {
                return summary.clone();
            }
        }
        let summary = FolderDashboardSummary::calculate(path);
        *self.right_pane_dashboard_cache.borrow_mut() = Some((path.to_path_buf(), summary.clone()));
        summary
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
        // Start with top-level items from the current directory, sorted according to active sort mode
        let mut top_level = self.get_children(&self.current_path);
        top_level.reverse(); // reverse for stack popping order
        let mut stack: Vec<(DirEntry, u32)> = top_level.into_iter().map(|e| (e, 0)).collect();

        while let Some((entry, depth)) = stack.pop() {
            result.push((entry.clone(), depth));
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

    pub fn sync_navigation_items(&mut self) {
        if self.navigation.filter_query().is_none() {
            let tree = self.build_expanded_tree();
            let flat_items: Vec<DirEntry> = tree.into_iter().map(|(entry, _)| entry).collect();
            let selected_path = self.navigation.current_selection().map(|e| e.path.clone());
            self.navigation.update_items_raw(flat_items);
            if let Some(path) = selected_path {
                self.navigation.select_path(&path);
            }
        }
    }

    /// Descend into the currently selected sub-directory and rescan inside the active tab.
    pub fn navigate_in(&mut self) {
        if let Some(target) = self.selected_item() {
            if target.entry_type == EntryType::Directory {
                let path = target.path;
                let (active_stats, scan_progress, git_ctx, immediate_entries) =
                    Self::prepare_directory_load(&path);

                self.current_path = path.clone();
                self.active_stats = active_stats;
                self.scan_progress = scan_progress;
                self.navigation.update_items(immediate_entries.clone());
                self.navigation.set_cursor(0);
                self.navigation.clear_expanded();
                self.navigation.set_filter(None, &[]);
                self.all_entries = immediate_entries;
                self.scan_applied = false;
                self.preview_cache = None;
                self.git_ctx = git_ctx;

                self.record_navigation(path.clone());
                self.save_active_tab();
                self.reset_selection_timeout();

                if let Some(ref engine) = self.plugin_engine {
                    let _ = engine.trigger_event("on_enter", path.to_string_lossy().to_string());
                }
            }
        }
    }

    /// Ascend to the parent directory and rescan inside the active tab.
    pub fn navigate_out(&mut self) {
        if let Some(parent) = self.current_path.parent() {
            let path = parent.to_path_buf();
            let (active_stats, scan_progress, git_ctx, immediate_entries) =
                Self::prepare_directory_load(&path);

            self.current_path = path.clone();
            self.active_stats = active_stats;
            self.scan_progress = scan_progress;
            self.navigation.update_items(immediate_entries.clone());
            self.navigation.set_cursor(0);
            self.navigation.clear_expanded();
            self.navigation.set_filter(None, &[]);
            self.all_entries = immediate_entries;
            self.scan_applied = false;
            self.preview_cache = None;
            self.git_ctx = git_ctx;

            self.record_navigation(path.clone());
            self.save_active_tab();
            self.reset_selection_timeout();

            if let Some(ref engine) = self.plugin_engine {
                let _ = engine.trigger_event("on_enter", path.to_string_lossy().to_string());
            }
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

    pub fn reset_selection_timeout(&mut self) {
        self.last_selection_time = std::time::Instant::now();
        if let Some(item) = self.navigation.current_selection() {
            if let Some(ref engine) = self.plugin_engine {
                let _ = engine.trigger_event("on_file_select", item.path.to_string_lossy().to_string());
            }
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
            *self.last_rendered_image.lock().unwrap() = None;
            self.reset_selection_timeout();

            if let Some(ref engine) = self.plugin_engine {
                let _ = engine.trigger_event("on_tab_change", (index + 1).to_string());
            }
        }
    }

    /// Open a new tab at the specified path and switch to it.
    pub fn open_tab(&mut self, path: PathBuf) {
        self.save_active_tab();

        let (active_stats, scan_progress, git_ctx, immediate_entries) =
            Self::prepare_directory_load(&path);

        let new_tab = Tab {
            current_path: path,
            active_stats,
            scan_progress,
            navigation: crate::navigation::Navigation::new(
                immediate_entries.clone(),
                SortMode::SizeDesc,
            ),
            all_entries: immediate_entries,
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

    /// Close the tab at a specific index. Reject if it is the last tab or index out of range.
    pub fn close_tab_at(&mut self, index: usize) {
        if self.tabs.len() <= 1 || index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
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
        let (active_stats, scan_progress, git_ctx, immediate_entries) =
            Self::prepare_directory_load(&path);

        self.current_path = path.clone();
        self.active_stats = active_stats;
        self.scan_progress = scan_progress;
        self.navigation.update_items(immediate_entries.clone());
        self.navigation.set_cursor(0);
        self.navigation.clear_expanded();
        self.navigation.set_filter(None, &[]);
        self.all_entries = immediate_entries;
        self.scan_applied = false;
        self.preview_cache = None;
        self.git_ctx = git_ctx;

        self.record_navigation(path.clone());
        self.save_active_tab();
        self.reset_selection_timeout();

        if let Some(ref engine) = self.plugin_engine {
            let _ = engine.trigger_event("on_enter", path.to_string_lossy().to_string());
        }
    }

    pub fn execute_plugin_command(&mut self, cmd: crate::plugin::commands::PluginCommand) {
        match cmd {
            crate::plugin::commands::PluginCommand::FocusPath { path } => {
                self.jump_to_path(std::path::PathBuf::from(path));
            }
            crate::plugin::commands::PluginCommand::ExecShell { cmd } => {
                // Execute shell command in background
                let _ = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
            }
            crate::plugin::commands::PluginCommand::None => {}
        }
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
        self.reset_selection_timeout();
    }

    /// Toggle search-mode focus. Leaving search mode preserves the query so the
    /// user can inspect the filtered results before clearing it.
    pub fn toggle_search_mode(&mut self) {
        self.search_mode = !self.search_mode;
    }

    /// Clear the active query and reset the cursor to the first visible row.
    pub fn clear_search(&mut self) {
        self.navigation.set_filter(None, &self.all_entries);
        self.sync_navigation_items();
        self.reset_selection_timeout();
    }

    /// Append one typed character to the live query.
    pub fn push_search_char(&mut self, ch: char) {
        let mut query = self.navigation.filter_query().unwrap_or("").to_string();
        query.push(ch);
        self.navigation.set_filter(Some(query), &self.all_entries);
        self.reset_selection_timeout();
    }

    /// Delete the most recent search character.
    pub fn pop_search_char(&mut self) {
        let mut query = self.navigation.filter_query().unwrap_or("").to_string();
        query.pop();
        if query.is_empty() {
            self.navigation.set_filter(None, &self.all_entries);
            self.sync_navigation_items();
        } else {
            self.navigation.set_filter(Some(query), &self.all_entries);
        }
        self.reset_selection_timeout();
    }

    pub fn update_search_overlay_results(&mut self) {
        if self.search_overlay_input.is_empty() {
            // Return all files by default if search query is empty
            self.search_overlay_results = self
                .all_entries
                .iter()
                .filter(|e| e.entry_type == EntryType::File)
                .map(|e| SearchMatch {
                    path: e.path.clone(),
                    line_number: None,
                    text: e
                        .path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                })
                .collect();
            self.search_overlay_selected_index = 0;
            return;
        }

        match self.search_overlay_mode {
            SearchOverlayMode::FuzzyFiles => {
                use nucleo::{
                    pattern::{CaseMatching, Normalization, Pattern},
                    Config, Matcher,
                };

                let mut matcher = Matcher::new(Config::DEFAULT);
                let pattern = Pattern::parse(
                    &self.search_overlay_input,
                    CaseMatching::Smart,
                    Normalization::Smart,
                );

                let mut results = Vec::new();
                for item in &self.all_entries {
                    if item.entry_type == EntryType::File {
                        let filename = item.path.file_name().unwrap_or_default().to_string_lossy();
                        let haystack = nucleo::Utf32String::from(filename.as_ref());
                        if let Some(score) = pattern.score(haystack.slice(..), &mut matcher) {
                            results.push((item.clone(), score));
                        }
                    }
                }
                results.sort_by_key(|b| std::cmp::Reverse(b.1));

                self.search_overlay_results = results
                    .into_iter()
                    .map(|(e, _)| SearchMatch {
                        path: e.path.clone(),
                        line_number: None,
                        text: e
                            .path
                            .file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into_owned(),
                    })
                    .collect();
                self.search_overlay_selected_index = 0;
            }
            SearchOverlayMode::LiveGrep => {
                self.search_overlay_results.clear();
                self.search_overlay_selected_index = 0;
                let _ = self
                    .rg_query_tx
                    .send(crate::search::ripgrep::RgSearchQuery {
                        query: self.search_overlay_input.clone(),
                        dir: self.current_path.clone(),
                    });
            }
        }
    }

    pub fn update_plugin_modal_filtering(&mut self) {
        let query = self.plugin_modal_input.to_lowercase();
        if query.is_empty() {
            self.plugin_modal_filtered_items = self.plugin_modal_items.clone();
        } else {
            self.plugin_modal_filtered_items = self.plugin_modal_items
                .iter()
                .filter(|item| item.label.to_lowercase().contains(&query))
                .cloned()
                .collect();
        }
        self.plugin_modal_selected_index = 0;
    }

    pub fn update_command_palette_results(&mut self) {
        if self.command_palette_input.starts_with('!') {
            let cmd_to_run = self.command_palette_input[1..].trim().to_string();
            if cmd_to_run.is_empty() {
                self.command_palette_results = vec![crate::project::detector::DetectedCommand {
                    name: "Enter a shell command to execute...".to_string(),
                    cmd: "".to_string(),
                    source: "Shell".to_string(),
                }];
            } else {
                self.command_palette_results = vec![crate::project::detector::DetectedCommand {
                    name: format!("Run: {}", cmd_to_run),
                    cmd: cmd_to_run.clone(),
                    source: "Shell".to_string(),
                }];
            }
            self.command_palette_selected_index = 0;
            return;
        }

        if self.command_palette_input.is_empty() {
            self.command_palette_results = self.command_palette_candidates.clone();
            self.command_palette_selected_index = 0;
            return;
        }

        use nucleo::{
            pattern::{CaseMatching, Normalization, Pattern},
            Config, Matcher,
        };

        let mut matcher = Matcher::new(Config::DEFAULT);
        let pattern = Pattern::parse(
            &self.command_palette_input,
            CaseMatching::Smart,
            Normalization::Smart,
        );

        let mut results = Vec::new();
        for item in &self.command_palette_candidates {
            let haystack = nucleo::Utf32String::from(item.name.as_str());
            if let Some(score) = pattern.score(haystack.slice(..), &mut matcher) {
                results.push((item.clone(), score));
            }
        }
        results.sort_by_key(|b| std::cmp::Reverse(b.1));

        self.command_palette_results = results.into_iter().map(|(item, _)| item).collect();
        self.command_palette_selected_index = 0;
    }

    pub fn rebuild_command_palette_candidates(&mut self) {
        let root = &self.current_path;
        let mut candidates = crate::project::detector::detect_project_commands(root);
        let session_cfg = crate::config::session::parse_session_config(root);
        for custom in session_cfg.commands {
            candidates.push(crate::project::detector::DetectedCommand {
                name: custom.name,
                cmd: custom.cmd,
                source: ".ascope.toml".to_string(),
            });
        }
        candidates.push(crate::project::detector::DetectedCommand {
            name: "Reload Plugins".to_string(),
            cmd: "reload_plugins".to_string(),
            source: "System".to_string(),
        });

        self.command_palette_candidates = candidates;
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
        assert_eq!(state.navigation.visible_items().len(), 3);
        assert_eq!(state.navigation.cursor(), 0);
    }

    #[test]
    fn test_move_selection_wraps_forward() {
        let (mut state, _dir) = make_state_with_subdirs();
        state.navigation.set_cursor(2); // last item
        state.move_selection(1);
        assert_eq!(state.navigation.cursor(), 0); // must wrap to first
    }

    #[test]
    fn test_move_selection_wraps_backward() {
        let (mut state, _dir) = make_state_with_subdirs();
        state.navigation.set_cursor(0);
        state.move_selection(-1);
        assert_eq!(state.navigation.cursor(), 2); // must wrap to last
    }

    #[test]
    fn test_move_selection_arbitrary_delta() {
        let (mut state, _dir) = make_state_with_subdirs(); // Has 3 items (indices 0, 1, 2)
        state.navigation.set_cursor(1);
        state.move_selection(5); // 1 + 5 = 6 -> 6 % 3 = 0
        assert_eq!(state.navigation.cursor(), 0);

        state.navigation.set_cursor(1);
        state.move_selection(-5); // 1 - 5 = -4 -> -4 % 3 = -1 -> -1 + 3 = 2
        assert_eq!(state.navigation.cursor(), 2);
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
        state.navigation.select_path(&src_dir);
        state.toggle_expand();

        state
            .navigation
            .set_filter(Some(String::from("main")), &state.all_entries);
        let filtered = state.visible_items();
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
        state.navigation.select_path(&src_dir);
        state.toggle_expand();
        state
            .navigation
            .set_filter(Some(String::from("app")), &state.all_entries);

        let visible = state.visible_items();
        assert_eq!(visible.len(), 1);
        assert!(visible[0].0.path.ends_with(PathBuf::from("src/app.rs")));
    }

    #[test]
    fn test_search_editing_resets_selection() {
        let dir = tempdir().unwrap();
        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);
        state.navigation.update_items(vec![
            mock_dir_entry(PathBuf::from("alpha"), 1, true),
            mock_dir_entry(PathBuf::from("beta"), 1, true),
        ]);
        state.navigation.set_cursor(1);

        state.push_search_char('a');
        assert_eq!(state.navigation.cursor(), 0);
        assert_eq!(state.navigation.filter_query().unwrap_or(""), "a");

        state.pop_search_char();
        assert_eq!(state.navigation.cursor(), 0);
        assert!(state.navigation.filter_query().unwrap_or("").is_empty());
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
        state.navigation.select_path(&src_dir);
        state.toggle_expand();
        state
            .navigation
            .set_filter(Some(String::from("main.rs")), &state.all_entries);
        let filtered = state.visible_items();

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
        assert_eq!(state.navigation.visible_items().len(), 1);
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

        state
            .navigation
            .update_items(vec![mock_dir_entry(file_path.clone(), 12, false)]);
        state.navigation.set_cursor(0);

        assert!(state.preview_lines().is_empty());

        state.update_preview_cache(80, 24);

        let lines = state.preview_lines();
        assert!(!lines.is_empty());
        assert!(lines[0].to_string().contains("fn test()"));

        state.navigation.update_items(vec![]);
        state.update_preview_cache(80, 24);
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

        state
            .navigation
            .set_filter(Some(String::from("mrs")), &state.all_entries);
        let matches_mrs = state.visible_items();
        assert_eq!(matches_mrs.len(), 1);
        assert_eq!(matches_mrs[0].0.path, main_path);

        state
            .navigation
            .set_filter(Some(String::from("crgo")), &state.all_entries);
        let matches_crgo = state.visible_items();
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

        state.navigation.set_filter(None, &state.all_entries);
        let filtered = state.visible_items();
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_no_match_returns_empty() {
        let dir = tempdir().unwrap();
        let f1 = dir.path().join("a.rs");
        File::create(&f1).unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        state
            .navigation
            .set_filter(Some(String::from("nonexistent")), &state.all_entries);
        let filtered = state.visible_items();
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
            display_path: "z_file".to_string(),
            symlink_target: None,
        };
        let e2 = DirEntry {
            path: PathBuf::from("a_file"),
            size: 100,
            entry_type: EntryType::Directory,
            mtime: t2,
            display_path: "a_file".to_string(),
            symlink_target: None,
        };

        state.navigation.update_items(vec![e1.clone(), e2.clone()]);

        assert_eq!(
            state.navigation.visible_items()[0].path,
            PathBuf::from("a_file")
        );

        state.navigation.set_sort_mode(SortMode::NameAsc);
        assert_eq!(
            state.navigation.visible_items()[0].path,
            PathBuf::from("a_file")
        );

        state.navigation.set_sort_mode(SortMode::MtimeDesc);
        assert_eq!(
            state.navigation.visible_items()[0].path,
            PathBuf::from("a_file")
        );

        state.navigation.set_sort_mode(SortMode::SizeDesc);
        state.cycle_sort_mode();
        assert_eq!(state.navigation.sort_mode(), SortMode::NameAsc);
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
        state.navigation.set_cursor(0);
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

        state
            .navigation
            .update_items(vec![mock_dir_entry(f1.clone(), 0, false)]);
        state.navigation.set_cursor(0);

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
        state
            .navigation
            .update_items(vec![mock_dir_entry(file_path.clone(), 12, false)]);
        state.navigation.set_cursor(0);

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

        state
            .navigation
            .update_items(vec![mock_dir_entry(file_path.clone(), 11, false)]);
        state.navigation.set_cursor(0);

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

        state
            .navigation
            .update_items(vec![mock_dir_entry(file_path.clone(), 14, false)]);
        state.navigation.set_cursor(0);

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

        state
            .navigation
            .update_items(vec![mock_dir_entry(file_path.clone(), 0, false)]);
        state.navigation.set_cursor(0);

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

    #[test]
    fn test_tree_cursor_movement_and_selection() {
        let dir = tempdir().unwrap();
        let sub1 = dir.path().join("dir1");
        let sub2 = dir.path().join("dir2");
        std::fs::create_dir(&sub1).unwrap();
        std::fs::create_dir(&sub2).unwrap();

        let file1 = sub1.join("file1.txt");
        {
            let mut f = File::create(&file1).unwrap();
            f.write_all(b"content").unwrap();
        }

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        // Ensure NameAsc sort mode to keep order predictable: dir1 then dir2
        state.navigation.set_sort_mode(SortMode::NameAsc);

        // Initial visible items should be dir1, dir2
        let visible = state.visible_items();
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].0.path, sub1);
        assert_eq!(visible[1].0.path, sub2);

        // Cursor starts at 0 (selecting dir1)
        assert_eq!(state.navigation.cursor(), 0);
        assert_eq!(state.selected_item().unwrap().path, sub1);

        // Toggle expand dir1
        state.toggle_expand();

        // Now visible items should be dir1, dir1/file1.txt, dir2
        let visible_expanded = state.visible_items();
        assert_eq!(visible_expanded.len(), 3);
        assert_eq!(visible_expanded[0].0.path, sub1);
        assert_eq!(visible_expanded[1].0.path, file1);
        assert_eq!(visible_expanded[2].0.path, sub2);

        // Cursor should still be on dir1 (0)
        assert_eq!(state.navigation.cursor(), 0);
        assert_eq!(state.selected_item().unwrap().path, sub1);

        // Move cursor down -> should select file1.txt (1)
        state
            .navigation
            .move_cursor(crate::navigation::Direction::Down);
        assert_eq!(state.navigation.cursor(), 1);
        assert_eq!(state.selected_item().unwrap().path, file1);

        // Move cursor down -> should select dir2 (2)
        state
            .navigation
            .move_cursor(crate::navigation::Direction::Down);
        assert_eq!(state.navigation.cursor(), 2);
        assert_eq!(state.selected_item().unwrap().path, sub2);

        // Move cursor down again -> should stay at dir2 (2)
        state
            .navigation
            .move_cursor(crate::navigation::Direction::Down);
        assert_eq!(state.navigation.cursor(), 2);
        assert_eq!(state.selected_item().unwrap().path, sub2);
    }

    #[test]
    fn test_image_and_pdf_preview_type_detection() {
        let dir = tempdir().unwrap();
        let state = AppState::new(dir.path().to_path_buf());

        assert_eq!(
            state.detect_preview_type(std::path::Path::new("image.png")),
            PreviewType::Image
        );
        assert_eq!(
            state.detect_preview_type(std::path::Path::new("doc.pdf")),
            PreviewType::Image
        );
        assert_eq!(
            state.detect_preview_type(std::path::Path::new("text.txt")),
            PreviewType::Text
        );
    }

    #[test]
    fn test_image_preview_pipeline_integration() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_image.png");
        let mut f = File::create(&file_path).unwrap();
        f.write_all(b"NOT_A_REAL_PNG_DATA").unwrap();

        let mut state = AppState::new(dir.path().to_path_buf());
        wait_for_scan(&mut state);

        state
            .navigation
            .update_items(vec![mock_dir_entry(file_path.clone(), 12, false)]);
        state.navigation.set_cursor(0);

        state.update_preview_cache(80, 24);

        let lines = state.preview_lines();
        assert!(!lines.is_empty());
        assert!(lines[0].to_string().contains("[Loading preview...]"));

        let mut success = false;
        for _ in 0..500 {
            std::thread::sleep(std::time::Duration::from_millis(10));
            state.poll_preview_updates();
            let updated_lines = state.preview_lines();
            if !updated_lines.is_empty()
                && !updated_lines[0]
                    .to_string()
                    .contains("[Loading preview...]")
            {
                assert!(
                    updated_lines[0].to_string().contains("Error loading image")
                        || updated_lines[0].to_string().contains("Preview error")
                );
                success = true;
                break;
            }
        }
        assert!(success, "Preview update timed out");
    }
}
