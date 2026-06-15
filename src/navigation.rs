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

#[derive(Clone)]
struct FilterCache {
    query: String,
    results: Vec<(usize, u32)>,
}

impl Default for FilterCache {
    fn default() -> Self {
        Self {
            query: String::new(),
            results: Vec::new(),
        }
    }
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
                self.items.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.path.cmp(&b.path)));
            }
            crate::app::SortMode::NameAsc => {
                self.items.sort_by(|a, b| {
                    let name_a = a.path.file_name().unwrap_or_default();
                    let name_b = b.path.file_name().unwrap_or_default();
                    name_a.cmp(name_b)
                });
            }
            crate::app::SortMode::MtimeDesc => {
                self.items.sort_by(|a, b| b.mtime.cmp(&a.mtime).then_with(|| a.path.cmp(&b.path)));
            }
        }
    }
}
