// ==========================================================================
// File    : git.rs
// Project : AuraScope
// Layer   : Git
// Purpose : Detects local git repositories and reads branch and dirty file details.
//
// Author  : Ahmed Ashour
// Created : 2026-06-14
// ==========================================================================

use git2::Repository;
use std::path::Path;

/// Git repository context for status bar rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitContext {
    /// Name of the active branch.
    pub branch: String,
    /// Number of untracked, modified, or staged files.
    pub dirty_count: usize,
}

impl GitContext {
    /// Read git repository context starting from `path` walking up parents.
    /// Returns `None` if `path` is not inside a git repository.
    pub fn read(path: &Path) -> Option<Self> {
        let repo = Repository::discover(path).ok()?;

        let branch = match repo.head() {
            Ok(head) => {
                if repo.head_detached().unwrap_or(false) {
                    if let Some(target_id) = head.target() {
                        let hex = target_id.to_string();
                        if hex.len() >= 7 {
                            format!("detached@{}", &hex[..7])
                        } else {
                            "detached".to_string()
                        }
                    } else {
                        head.shorthand().unwrap_or("detached").to_string()
                    }
                } else {
                    head.shorthand().unwrap_or("unknown").to_string()
                }
            }
            Err(_) => {
                let target = repo
                    .find_reference("HEAD")
                    .ok()
                    .and_then(|r| r.symbolic_target().map(|s| s.to_string()));
                match target {
                    Some(t) => t.strip_prefix("refs/heads/").unwrap_or(&t).to_string(),
                    None => "main".to_string(),
                }
            }
        };

        let dirty_count = if repo.is_bare() {
            0
        } else {
            let mut opts = git2::StatusOptions::new();
            opts.include_untracked(true);
            opts.recurse_untracked_dirs(false);
            opts.renames_head_to_index(false);
            opts.renames_index_to_workdir(false);
            repo.statuses(Some(&mut opts)).map(|s| s.len()).unwrap_or(0)
        };

        Some(Self {
            branch,
            dirty_count,
        })
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

    #[test]
    fn test_git_context_non_git_dir() {
        let dir = tempdir().unwrap();
        // A newly created tempdir is not a git repo
        assert!(GitContext::read(dir.path()).is_none());
    }

    #[test]
    fn test_git_context_empty_repo() {
        let dir = tempdir().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        // Verify it reads symbolic target for unborn branch
        let ctx = GitContext::read(dir.path()).unwrap();
        // Git default branch name is usually "master" or "main"
        assert!(ctx.branch == "master" || ctx.branch == "main");
        assert_eq!(ctx.dirty_count, 0);
    }

    #[test]
    fn test_git_context_with_dirty_files() {
        let dir = tempdir().unwrap();
        let _repo = Repository::init(dir.path()).unwrap();

        // Create dirty files
        {
            let mut f1 = File::create(dir.path().join("untracked.txt")).unwrap();
            f1.write_all(b"new file").unwrap();
        }

        let ctx = GitContext::read(dir.path()).unwrap();
        assert_eq!(ctx.dirty_count, 1);
    }

    #[test]
    fn test_git_context_bare_repo() {
        let dir = tempdir().unwrap();
        let _repo = Repository::init_bare(dir.path()).unwrap();

        let ctx = GitContext::read(dir.path()).unwrap();
        assert_eq!(ctx.dirty_count, 0);
    }

    #[test]
    fn test_git_context_detached_head() {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Configure dummy user for committing
        {
            let mut config = repo.config().unwrap();
            config.set_str("user.name", "Test").unwrap();
            config.set_str("user.email", "test@example.com").unwrap();
        }

        // Create initial commit
        let commit_id = {
            let mut index = repo.index().unwrap();
            let file_path = dir.path().join("dummy.txt");
            File::create(&file_path).unwrap();
            index.add_path(Path::new("dummy.txt")).unwrap();
            index.write().unwrap();

            let tree_id = index.write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            let sig = repo.signature().unwrap();
            repo.commit(Some("HEAD"), &sig, &sig, "initial commit", &tree, &[])
                .unwrap()
        };

        // Detach HEAD
        repo.set_head_detached(commit_id).unwrap();

        let ctx = GitContext::read(dir.path()).unwrap();
        assert!(ctx.branch.starts_with("detached@"));
        assert_eq!(ctx.dirty_count, 0);
    }
}
