#![allow(dead_code)]

use crate::fs::walker::{DirEntry, EntryType};
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
pub struct Navigation {
    items: Vec<DirEntry>,
    cursor: usize,
    expanded: HashSet<PathBuf>,
    filter_query: Option<String>,
    sort_mode: crate::app::SortMode,
    filtered_items: Option<Vec<(DirEntry, u32)>>,
}

impl Navigation {
    pub fn new(items: Vec<DirEntry>, sort_mode: crate::app::SortMode) -> Self {
        let mut nav = Self {
            items,
            cursor: 0,
            expanded: HashSet::new(),
            filter_query: None,
            sort_mode,
            filtered_items: None,
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

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: usize) {
        let max_idx = self.visible_items().len().saturating_sub(1);
        self.cursor = cursor.min(max_idx);
    }

    pub fn visible_items(&self) -> Vec<&DirEntry> {
        if let Some(ref filtered) = self.filtered_items {
            filtered.iter().map(|(entry, _)| entry).collect()
        } else {
            self.items.iter().collect()
        }
    }

    pub fn visible_items_with_scores(&self) -> Vec<(&DirEntry, u32)> {
        if let Some(ref filtered) = self.filtered_items {
            filtered
                .iter()
                .map(|(entry, score)| (entry, *score))
                .collect()
        } else {
            self.items.iter().map(|entry| (entry, 0)).collect()
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

    pub fn clear_expanded(&mut self) {
        self.expanded.clear();
    }

    pub fn set_filter(&mut self, query: Option<String>, all_entries: &[DirEntry]) {
        self.filter_query = query.clone();

        if let Some(q) = query {
            use nucleo::{
                pattern::{CaseMatching, Normalization, Pattern},
                Config, Matcher,
            };

            let mut matcher = Matcher::new(Config::DEFAULT);
            let pattern = Pattern::parse(&q, CaseMatching::Smart, Normalization::Smart);

            let mut results = Vec::new();
            for item in all_entries {
                let haystack = nucleo::Utf32String::from(item.display_path.as_str());
                if let Some(score) = pattern.score(haystack.slice(..), &mut matcher) {
                    results.push((item.clone(), score));
                }
            }
            results.sort_by_key(|b| std::cmp::Reverse(b.1));

            self.filtered_items = Some(results);
            self.cursor = 0;
        } else {
            self.filtered_items = None;
            self.cursor = 0;
        }
    }

    pub fn filter_query(&self) -> Option<&str> {
        self.filter_query.as_deref()
    }

    pub fn set_sort_mode(&mut self, mode: crate::app::SortMode) {
        self.sort_mode = mode;
        self.apply_sort();
        self.cursor = 0;
    }

    pub fn sort_mode(&self) -> crate::app::SortMode {
        self.sort_mode
    }

    pub fn update_items(&mut self, items: Vec<DirEntry>) {
        self.items = items;
        self.apply_sort();
        let visible_count = self.visible_items().len();
        if self.cursor >= visible_count && visible_count > 0 {
            self.cursor = visible_count - 1;
        }
        self.filtered_items = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::SystemTime;

    fn mock_entry(name: &str) -> DirEntry {
        DirEntry {
            path: PathBuf::from(name),
            size: 0,
            entry_type: EntryType::File,
            mtime: SystemTime::now(),
            display_path: name.to_string(),
            symlink_target: None,
        }
    }

    fn mock_dir_entry(name: &str) -> DirEntry {
        DirEntry {
            path: PathBuf::from(name),
            size: 0,
            entry_type: EntryType::Directory,
            mtime: SystemTime::now(),
            display_path: name.to_string(),
            symlink_target: None,
        }
    }

    fn mock_entry_sized(name: &str, size: u64) -> DirEntry {
        DirEntry {
            path: PathBuf::from(name),
            size,
            entry_type: EntryType::File,
            mtime: SystemTime::now(),
            display_path: name.to_string(),
            symlink_target: None,
        }
    }

    #[test]
    fn test_cursor_moves_down() {
        let items = vec![mock_entry("a"), mock_entry("b"), mock_entry("c")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        nav.move_cursor(Direction::Down);
        assert_eq!(nav.cursor, 1);
    }

    #[test]
    fn test_cursor_stays_at_bottom() {
        let items = vec![mock_entry("a"), mock_entry("b")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);
        nav.cursor = 1;

        nav.move_cursor(Direction::Down);
        assert_eq!(nav.cursor, 1);
    }

    #[test]
    fn test_cursor_moves_up() {
        let items = vec![mock_entry("a"), mock_entry("b"), mock_entry("c")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);
        nav.cursor = 2;

        nav.move_cursor(Direction::Up);
        assert_eq!(nav.cursor, 1);
    }

    #[test]
    fn test_cursor_first_last() {
        let items = vec![mock_entry("a"), mock_entry("b"), mock_entry("c")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        nav.move_cursor(Direction::Last);
        assert_eq!(nav.cursor, 2);

        nav.move_cursor(Direction::First);
        assert_eq!(nav.cursor, 0);
    }

    #[test]
    fn test_filter_narrows_visible_items() {
        let items = vec![
            mock_entry("src/main.rs"),
            mock_entry("tests/test.rs"),
            mock_entry("README.md"),
        ];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        let all = nav.items.clone();
        nav.set_filter(Some("src".to_string()), &all);
        assert_eq!(nav.visible_items().len(), 1);
        assert!(nav.visible_items()[0].display_path.contains("src"));
    }

    #[test]
    fn test_clear_filter_shows_all() {
        let items = vec![mock_entry("src/main.rs"), mock_entry("tests/test.rs")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        let all = nav.items.clone();
        nav.set_filter(Some("src".to_string()), &all);
        assert_eq!(nav.visible_items().len(), 1);

        nav.set_filter(None, &all);
        assert_eq!(nav.visible_items().len(), 2);
    }

    #[test]
    fn test_enter_directory_returns_action() {
        let items = vec![mock_dir_entry("subdir")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        let action = nav.enter_selected();
        match action {
            NavigationAction::EnterDirectory(path) => assert!(path.ends_with("subdir")),
            _ => panic!("Expected EnterDirectory action"),
        }
    }

    #[test]
    fn test_enter_file_returns_action() {
        let items = vec![mock_entry("file.txt")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        let action = nav.enter_selected();
        match action {
            NavigationAction::OpenFile(path) => assert!(path.ends_with("file.txt")),
            _ => panic!("Expected OpenFile action"),
        }
    }

    #[test]
    fn test_toggle_expansion() {
        let items = vec![mock_dir_entry("subdir")];
        let mut nav = Navigation::new(items, crate::app::SortMode::NameAsc);

        let path = PathBuf::from("subdir");
        assert!(!nav.is_expanded(&path));

        nav.toggle_expand_selected();
        assert!(nav.is_expanded(&path));

        nav.toggle_expand_selected();
        assert!(!nav.is_expanded(&path));
    }

    #[test]
    fn test_sorting_by_size() {
        let items = vec![
            mock_entry_sized("large", 1000),
            mock_entry_sized("small", 10),
            mock_entry_sized("medium", 500),
        ];
        let nav = Navigation::new(items, crate::app::SortMode::SizeDesc);

        let visible = nav.visible_items();
        assert_eq!(visible[0].size, 1000);
        assert_eq!(visible[1].size, 500);
        assert_eq!(visible[2].size, 10);
    }
}
