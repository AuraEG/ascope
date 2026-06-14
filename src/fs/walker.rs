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

/// Aggregated size statistics for a scanned directory tree.
#[derive(Debug, Clone, Default, Serialize)]
pub struct PathStats {
    /// Total byte size of all files under the root.
    pub total_size: u64,
    /// Total number of regular files found.
    pub file_count: u64,
    /// Byte size attributed to each direct child directory.
    pub subdirs: HashMap<PathBuf, u64>,
    /// All entries (path, size, display_path) scanned recursively under the root path.
    pub all_entries: Vec<(PathBuf, u64, String)>,
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

/// Kick off a background thread that calls [`scan_path`] and writes results
/// into the supplied shared handles.  Returns immediately; the caller must
/// poll `progress_out` to know when the data is ready.
pub fn scan_path_async(
    root: PathBuf,
    stats_out: Arc<Mutex<PathStats>>,
    progress_out: Arc<Mutex<ScanProgress>>,
) {
    let builder = thread::Builder::new().name("ascope-scanner".to_string());
    builder
        .spawn(move || {
            *progress_out.lock().unwrap_or_else(|e| e.into_inner()) = ScanProgress::Scanning;
            match scan_path(&root) {
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
/// [`PathStats`]. Uses jwalk's parallel work-stealing traversal internally.
pub fn scan_path(root: &Path) -> Result<PathStats, std::io::Error> {
    let mut total_size: u64 = 0;
    let mut file_count: u64 = 0;
    let mut subdirs: HashMap<PathBuf, u64> = HashMap::new();
    let mut all_entries: Vec<(PathBuf, u64, String)> = Vec::new();

    let mut temp_entries = Vec::new();
    let mut dir_sizes: HashMap<PathBuf, u64> = HashMap::new();

    for entry in WalkDir::new(root)
        .skip_hidden(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path == root {
            continue;
        }

        let is_file = entry.file_type().is_file();
        let size = if is_file {
            entry.metadata().map(|m| m.len()).unwrap_or(0)
        } else {
            0
        };

        if is_file {
            total_size += size;
            file_count += 1;

            // Add size to all ancestor directories up to `root`
            let mut parent = path.parent();
            while let Some(p) = parent {
                if !p.starts_with(root) || p == root {
                    break;
                }
                *dir_sizes.entry(p.to_path_buf()).or_insert(0) += size;
                parent = p.parent();
            }
        }

        temp_entries.push((path.to_path_buf(), is_file, size));
    }

    // Populate direct subdirectory sizes for `subdirs`
    for (path, is_file, _) in &temp_entries {
        if !is_file && path.parent() == Some(root) {
            let size = *dir_sizes.get(path).unwrap_or(&0);
            subdirs.insert(path.clone(), size);
        }
    }

    // Populate all_entries with actual sizes
    for (path, is_file, size) in temp_entries {
        let actual_size = if is_file {
            size
        } else {
            *dir_sizes.get(&path).unwrap_or(&0)
        };

        let display_path = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        all_entries.push((path, actual_size, display_path));
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

        let stats = scan_path(dir.path()).unwrap();
        assert_eq!(stats.total_size, 17);
        assert_eq!(stats.file_count, 2);
    }

    #[test]
    fn test_scan_empty_directory() {
        let dir = tempdir().unwrap();
        let stats = scan_path(dir.path()).unwrap();
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

        let stats = scan_path(dir.path()).unwrap();
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

        let stats = scan_path(dir.path()).unwrap();
        // In all_entries, both pkg and pkg/src should have size 12
        let pkg_entry = stats
            .all_entries
            .iter()
            .find(|(p, _, _)| p == &sub)
            .unwrap();
        assert_eq!(pkg_entry.1, 12);

        let pkg_src_entry = stats
            .all_entries
            .iter()
            .find(|(p, _, _)| p == &subsub)
            .unwrap();
        assert_eq!(pkg_src_entry.1, 12);
    }
}
