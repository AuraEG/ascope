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

    for entry in WalkDir::new(root)
        .skip_hidden(true)
        .into_iter()
        .filter_map(Result::ok)
    {
        // Only account for regular files; directories carry no payload size.
        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                let size = metadata.len();
                total_size += size;
                file_count += 1;

                // Attribute this file's size to its direct child subdirectory
                // so callers can rank top-level folders by disk usage.
                if let Ok(relative) = entry.path().strip_prefix(root) {
                    if let Some(first) = relative.components().next() {
                        let child = root.join(first.as_os_str());
                        if child.is_dir() {
                            *subdirs.entry(child).or_insert(0) += size;
                        }
                    }
                }
            }
        }
    }

    Ok(PathStats {
        total_size,
        file_count,
        subdirs,
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

        let mut f1 = File::create(file_path1).unwrap();
        f1.write_all(b"hello").unwrap(); // 5 bytes

        let mut f2 = File::create(file_path2).unwrap();
        f2.write_all(b"rust-systems").unwrap(); // 12 bytes

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
        let mut f = File::create(sub.join("lib.rs")).unwrap();
        f.write_all(b"fn main() {}").unwrap(); // 12 bytes

        let stats = scan_path(dir.path()).unwrap();
        assert_eq!(*stats.subdirs.get(&sub).unwrap(), 12);
    }
}
