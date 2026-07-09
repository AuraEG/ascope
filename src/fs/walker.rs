// ==========================================================================
// File    : fs/walker.rs
// Project : AuraScope
// Layer   : FileSystem
// Purpose : Multi-threaded parallel directory walker; aggregates size
//           statistics per entry using jwalk's work-stealing traversal.
//
// Author  : Ahmed Ashour
// Created : 2026-06-13
// ==========================================================================

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread;

use jwalk::WalkDir;
use serde::Serialize;

// --------------------------------------------------------------------------
// [SECTION] Public Types
// --------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum EntryType {
    File,
    Directory,
    Symlink,
}

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct DirEntry {
    pub path: PathBuf,
    pub size: u64,
    pub entry_type: EntryType,
    pub mtime: std::time::SystemTime,
    pub display_path: String,
    pub symlink_target: Option<PathBuf>,
}

/// Aggregated size statistics for a scanned directory tree.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PathStats {
    /// Total byte size of all files under the root.
    pub total_size: u64,
    /// Total number of regular files found.
    pub file_count: u64,
    /// Byte size attributed to each direct child directory.
    pub subdirs: HashMap<PathBuf, u64>,
    /// All entries scanned recursively under the root path.
    pub all_entries: Vec<DirEntry>,
}

// --------------------------------------------------------------------------
// [SECTION] Async Progress
// --------------------------------------------------------------------------

/// Tracks the lifecycle of a background directory scan.
#[derive(Debug, Clone, Default)]
pub enum ScanProgress {
    /// No scan has been started yet.
    #[default]
    Idle,
    /// A scan is in-flight.
    Scanning,
    /// The scan has finished (successfully or with an error).
    Complete,
}

/// Quickly scan immediate children of a directory (non-recursively).
/// Directory entry sizes are set to `u64::MAX` as a placeholder for uncalculated size.
pub fn scan_immediate(root: &Path, show_hidden: bool) -> Result<Vec<DirEntry>, std::io::Error> {
    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(root)?;
    for entry in read_dir {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if !show_hidden {
            // Skip hidden files/directories (starting with '.')
            if let Some(name) = path.file_name() {
                if name.to_string_lossy().starts_with('.') {
                    continue;
                }
            }
        }
        let file_type = match entry.file_type() {
            Ok(ft) => ft,
            Err(_) => continue,
        };
        let entry_type = if file_type.is_symlink() {
            EntryType::Symlink
        } else if file_type.is_dir() {
            EntryType::Directory
        } else {
            EntryType::File
        };

        let size = if entry_type == EntryType::File {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            u64::MAX // Placeholder for directories
        };

        let mtime = entry
            .metadata()
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let symlink_target = if entry_type == EntryType::Symlink {
            std::fs::read_link(&path).ok()
        } else {
            None
        };

        let display_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        entries.push(DirEntry {
            path,
            size,
            entry_type,
            mtime,
            display_path,
            symlink_target,
        });
    }
    Ok(entries)
}

/// Formats bytes into a human-readable size string (B, KB, MB, GB, TB).
pub fn format_size(bytes: u64) -> String {
    if bytes == u64::MAX {
        return "Calculating...".to_string();
    }
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    const TB: u64 = 1024 * 1024 * 1024 * 1024;

    #[allow(clippy::cast_precision_loss)]
    if bytes >= TB {
        format!("{:.2} TB", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

/// Kick off a background thread that calls [`scan_path`] and writes results
/// into the supplied shared handles.  Returns immediately; the caller must
/// poll `progress_out` to know when the data is ready.
pub fn scan_path_async(
    root: PathBuf,
    stats_out: Arc<Mutex<PathStats>>,
    progress_out: Arc<Mutex<ScanProgress>>,
    show_hidden: bool,
) {
    let builder = thread::Builder::new().name("ascope-scanner".to_string());
    builder
        .spawn(move || {
            *progress_out.lock().unwrap_or_else(|e| e.into_inner()) = ScanProgress::Scanning;
            match scan_path(&root, show_hidden) {
                Ok(stats) => {
                    *stats_out.lock().unwrap_or_else(|e| e.into_inner()) = stats;
                    *progress_out.lock().unwrap_or_else(|e| e.into_inner()) =
                        ScanProgress::Complete;
                }
                Err(_) => {
                    *progress_out.lock().unwrap_or_else(|e| e.into_inner()) =
                        ScanProgress::Complete;
                }
            }
        })
        .expect("Failed to spawn background scanner thread");
}

// --------------------------------------------------------------------------
// [SECTION] Scanner
// --------------------------------------------------------------------------

/// Recursively scan `root`, skipping hidden entries, and return aggregated
/// [`PathStats`]. Uses jwalk's serial traversal in the background to avoid CPU thread starvation.
pub fn scan_path(root: &Path, show_hidden: bool) -> Result<PathStats, std::io::Error> {
    let mut total_size: u64 = 0;
    let mut file_count: u64 = 0;
    let mut subdirs: HashMap<PathBuf, u64> = HashMap::new();
    let mut all_entries: Vec<DirEntry> = Vec::new();

    let mut temp_entries = Vec::new();
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();

    // 1. Walk the directory tree serially (caps CPU usage to 1 core, preventing lag)
    for entry in WalkDir::new(root)
        .skip_hidden(!show_hidden)
        .parallelism(jwalk::Parallelism::Serial)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root {
            continue;
        }

        let file_type = entry.file_type();
        let entry_type = if file_type.is_symlink() {
            EntryType::Symlink
        } else if file_type.is_dir() {
            EntryType::Directory
        } else {
            EntryType::File
        };

        let size = if entry_type == EntryType::File {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        let metadata = std::fs::symlink_metadata(&path);
        let mtime = metadata
            .ok()
            .and_then(|m| m.modified().ok())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH);

        let symlink_target = if entry_type == EntryType::Symlink {
            std::fs::read_link(&path).ok()
        } else {
            None
        };

        if entry_type == EntryType::File {
            total_size += size;
            file_count += 1;

            // Attribute file size directly to its immediate parent directory
            if let Some(parent) = path.parent() {
                if parent.starts_with(root) && parent != root {
                    *dir_sizes.entry(parent.to_path_buf()).or_insert(0) += size;
                }
            }
        }

        temp_entries.push((path.to_path_buf(), entry_type, size, mtime, symlink_target));
    }

    // 2. Propagate sizes upwards (from deepest subdirectories up to root).
    // This reduces hashmap lookups and heap allocations from O(N * D) to O(N + M)
    // where N is files and M is directories.
    let mut dirs: Vec<PathBuf> = temp_entries
        .iter()
        .filter(|(_, entry_type, _, _, _)| *entry_type == EntryType::Directory)
        .map(|(path, _, _, _, _)| path.clone())
        .collect();

    // Sort by path length descending (deepest directories first)
    dirs.sort_by_key(|p| std::cmp::Reverse(p.as_os_str().len()));

    for dir in dirs {
        let size = *dir_sizes.get(&dir).unwrap_or(&0);
        if let Some(parent) = dir.parent() {
            if parent.starts_with(root) && parent != root {
                *dir_sizes.entry(parent.to_path_buf()).or_insert(0) += size;
            }
        }
    }

    // Populate direct subdirectory sizes for `subdirs`
    for (path, entry_type, _, _, _) in &temp_entries {
        if *entry_type == EntryType::Directory && path.parent() == Some(root) {
            let size = *dir_sizes.get(path).unwrap_or(&0);
            subdirs.insert(path.clone(), size);
        }
    }

    // Populate all_entries with actual sizes and other details
    for (path, entry_type, size, mtime, symlink_target) in temp_entries {
        let actual_size = if entry_type == EntryType::Directory {
            *dir_sizes.get(&path).unwrap_or(&0)
        } else {
            size
        };

        let display_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        all_entries.push(DirEntry {
            path,
            size: actual_size,
            entry_type,
            mtime,
            display_path,
            symlink_target,
        });
    }

    Ok(PathStats {
        total_size,
        file_count,
        subdirs,
        all_entries,
    })
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

    #[test]
    fn test_scan_directory_sizes() {
        let dir = tempdir().unwrap();
        let file_path1 = dir.path().join("file1.txt");
        let sub_dir = dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        let file_path2 = sub_dir.join("file2.log");

        {
            let mut f1 = File::create(file_path1).unwrap();
            f1.write_all(b"hello").unwrap(); // 5 bytes
        }

        {
            let mut f2 = File::create(file_path2).unwrap();
            f2.write_all(b"rust-systems").unwrap(); // 12 bytes
        }

        let stats = scan_path(dir.path(), false).unwrap();
        assert_eq!(stats.total_size, 17);
        assert_eq!(stats.file_count, 2);
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let stats = scan_path(dir.path(), false).unwrap();
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.file_count, 0);
        assert!(stats.subdirs.is_empty());
    }

    #[test]
    fn test_subdirs_attribution() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("pkg");
        std::fs::create_dir(&sub).unwrap();
        {
            let mut f = File::create(sub.join("lib.rs")).unwrap();
            f.write_all(b"fn main() {}").unwrap(); // 12 bytes
        }

        let stats = scan_path(dir.path(), false).unwrap();
        assert_eq!(*stats.subdirs.get(&sub).unwrap(), 12);
    }

    #[test]
    fn test_async_scan_completes() {
        let dir = tempdir().unwrap();
        let mut f = File::create(dir.path().join("a.txt")).unwrap();
        f.write_all(b"hello").unwrap();

        let stats = Arc::new(Mutex::new(PathStats::default()));
        let progress = Arc::new(Mutex::new(ScanProgress::default()));
        scan_path_async(
            dir.path().to_path_buf(),
            Arc::clone(&stats),
            Arc::clone(&progress),
            false,
        );

        // Poll until complete (max 5 seconds).
        let start = std::time::Instant::now();
        loop {
            if matches!(*progress.lock().unwrap(), ScanProgress::Complete) {
                break;
            }
            assert!(start.elapsed().as_secs() < 5, "scan timed out");
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert_eq!(stats.lock().unwrap().file_count, 1);
    }

    #[test]
    fn test_recursive_directory_sizes_in_all_entries() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("pkg");
        let subsub = sub.join("src");
        std::fs::create_dir_all(&subsub).unwrap();
        {
            let mut f = File::create(subsub.join("lib.rs")).unwrap();
            f.write_all(b"fn main() {}").unwrap(); // 12 bytes
        }

        let stats = scan_path(dir.path(), false).unwrap();
        // In all_entries, both pkg and pkg/src should have size 12
        let pkg_entry = stats.all_entries.iter().find(|e| e.path == sub).unwrap();
        assert_eq!(pkg_entry.size, 12);

        let pkg_src_entry = stats.all_entries.iter().find(|e| e.path == subsub).unwrap();
        assert_eq!(pkg_src_entry.size, 12);
    }

    #[test]
    fn test_scan_immediate() {
        let dir = tempdir().unwrap();
        let file_path1 = dir.path().join("file1.txt");
        let sub_dir = dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();

        {
            let mut f1 = File::create(file_path1).unwrap();
            f1.write_all(b"hello").unwrap(); // 5 bytes
        }

        let entries = scan_immediate(dir.path(), false).unwrap();
        assert_eq!(entries.len(), 2);

        let file_entry = entries
            .iter()
            .find(|e| e.entry_type == EntryType::File)
            .unwrap();
        assert_eq!(file_entry.size, 5);

        let dir_entry = entries
            .iter()
            .find(|e| e.entry_type == EntryType::Directory)
            .unwrap();
        assert_eq!(dir_entry.size, u64::MAX);
    }
}
