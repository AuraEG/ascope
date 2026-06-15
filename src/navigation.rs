use crate::fs::walker::{DirEntry, EntryType};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    First,
    Last,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationAction {
    EnterDirectory(PathBuf),
    OpenFile(PathBuf),
    ToggleExpansion(PathBuf),
    None,
}

#[derive(Clone, Default)]
struct FilterCache {
    query: String,
    results: Vec<(usize, u32)>,
}

pub struct Navigation {
    items: Vec<DirEntry>,
    cursor: usize,
    expanded: HashSet<PathBuf>,
    filter_query: Option<String>,
    sort_mode: crate::app::SortMode,
    filter_cache: RefCell<FilterCache>,
}

impl Navigation {
    pub fn new(items: Vec<DirEntry>, sort_mode: crate::app::SortMode) -> Self {
        let mut nav = Self {
            items,
            cursor: 0,
            expanded: HashSet::new(),
            filter_query: None,
            sort_mode,
            filter_cache: RefCell::new(FilterCache::default()),
        };
        nav.apply_sort();
        nav
    }

    fn apply_sort(&mut self) {
        match self.sort_mode {
            crate::app::SortMode::SizeDesc => {
                self.items
                    .sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
            }
            crate::app::SortMode::NameAsc => {
                self.items.sort_by(|a, b| {
                    let name_a = a.path.file_name().unwrap_or_default();
                    let name_b = b.path.file_name().unwrap_or_default();
                    name_a.cmp(name_b)
                });
            }
            crate::app::SortMode::MtimeDesc => {
                self.items
                    .sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
            }
        }
    }

    pub fn move_cursor(&mut self, direction: Direction) -> NavigationAction {
        let visible = self.visible_items();
        if visible.is_empty() {
            return NavigationAction::None;
        }

        match direction {
            Direction::Up => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
            }
            Direction::Down => {
                if self.cursor < visible.len().saturating_sub(1) {
                    self.cursor += 1;
                }
            }
            Direction::First => {
                self.cursor = 0;
            }
            Direction::Last => {
                self.cursor = visible.len().saturating_sub(1);
            }
        }
        NavigationAction::None
    }

    pub fn visible_items(&self) -> Vec<&DirEntry> {
        if self.filter_query.is_some() {
            let cache = self.filter_cache.borrow();
            cache
                .results
                .iter()
                .filter_map(|(idx, _)| self.items.get(*idx))
                .collect()
        } else {
            self.items.iter().collect()
        }
    }

    pub fn current_selection(&self) -> Option<&DirEntry> {
        let visible = self.visible_items();
        visible.get(self.cursor).copied()
    }

    pub fn enter_selected(&mut self) -> NavigationAction {
        if let Some(entry) = self.current_selection() {
            match entry.entry_type {
                EntryType::Directory => NavigationAction::EnterDirectory(entry.path.clone()),
                EntryType::File => NavigationAction::OpenFile(entry.path.clone()),
                EntryType::Symlink => {
                    if let Some(target) = &entry.symlink_target {
                        if target.is_dir() {
                            NavigationAction::EnterDirectory(target.clone())
                        } else {
                            NavigationAction::OpenFile(entry.path.clone())
                        }
                    } else {
                        NavigationAction::None
                    }
                }
            }
        } else {
            NavigationAction::None
        }
    }

    pub fn toggle_expand_selected(&mut self) -> NavigationAction {
        if let Some(entry) = self.current_selection() {
            if entry.entry_type == EntryType::Directory {
                let path = entry.path.clone();
                if self.expanded.contains(&path) {
                    self.expanded.remove(&path);
                } else {
                    self.expanded.insert(path.clone());
                }
                return NavigationAction::ToggleExpansion(path);
            }
        }
        NavigationAction::None
    }

    pub fn is_expanded(&self, path: &Path) -> bool {
        self.expanded.contains(path)
    }

    pub fn set_filter(&mut self, query: Option<String>) {
        self.filter_query = query.clone();

        if let Some(q) = query {
            use nucleo::{
                pattern::{CaseMatching, Normalization, Pattern},
                Config, Matcher,
            };

            let mut matcher = Matcher::new(Config::DEFAULT);
            let pattern = Pattern::parse(&q, CaseMatching::Smart, Normalization::Smart);

            let mut results = Vec::new();
            for (idx, item) in self.items.iter().enumerate() {
                if let Some(score) = matcher.fuzzy_match(&item.display_path, &pattern) {
                    results.push((idx, score));
                }
            }
            results.sort_by(|a, b| b.1.cmp(&a.1));

            let mut cache = self.filter_cache.borrow_mut();
            cache.query = q;
            cache.results = results;

            self.cursor = 0;
        } else {
            self.filter_cache.borrow_mut().results.clear();
            self.cursor = 0;
        }
    }

    pub fn filter_query(&self) -> Option<&str> {
        self.filter_query.as_deref()
    }
}
