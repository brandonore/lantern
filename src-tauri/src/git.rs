use git2::Repository;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub detached: bool,
    pub ahead: usize,
    pub behind: usize,
}

pub fn git_info_for_path(path: &str) -> GitInfo {
    let repo = match Repository::discover(path) {
        Ok(r) => r,
        Err(_) => return GitInfo::default(),
    };

    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => {
            // No commits yet or other issue
            return GitInfo {
                branch: None,
                is_dirty: check_dirty(&repo),
                detached: false,
                ahead: 0,
                behind: 0,
            };
        }
    };

    let detached = repo.head_detached().unwrap_or(false);
    let branch = if detached {
        head.target()
            .map(|oid| format!("{:.7}", oid))
    } else {
        head.shorthand().map(|s| s.to_string())
    };

    let is_dirty = check_dirty(&repo);
    let (ahead, behind) = count_ahead_behind(&repo);

    GitInfo {
        branch,
        is_dirty,
        detached,
        ahead,
        behind,
    }
}

fn check_dirty(repo: &Repository) -> bool {
    let mut opts = git2::StatusOptions::new();
    opts.include_untracked(true)
        .recurse_untracked_dirs(false)
        .exclude_submodules(true);

    match repo.statuses(Some(&mut opts)) {
        Ok(statuses) => !statuses.is_empty(),
        Err(_) => false,
    }
}

fn count_ahead_behind(repo: &Repository) -> (usize, usize) {
    let head = match repo.head() {
        Ok(h) => h,
        Err(_) => return (0, 0),
    };

    let local_oid = match head.target() {
        Some(oid) => oid,
        None => return (0, 0),
    };

    let branch_name = match head.shorthand() {
        Some(name) => name.to_string(),
        None => return (0, 0),
    };

    let upstream_name = format!("refs/remotes/origin/{}", branch_name);
    let upstream_ref = match repo.find_reference(&upstream_name) {
        Ok(r) => r,
        Err(_) => return (0, 0),
    };

    let upstream_oid = match upstream_ref.target() {
        Some(oid) => oid,
        None => return (0, 0),
    };

    repo.graph_ahead_behind(local_oid, upstream_oid)
        .unwrap_or((0, 0))
}

// ── Worktree detection ──

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeEntry {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct WorktreeInfo {
    pub is_worktree: bool,
    pub repo_name: String,
    pub entries: Vec<WorktreeEntry>,
}

/// Detect worktree info for a given path.
/// Returns None if the path is not a git repo, or is a standalone repo with no worktrees.
pub fn detect_worktree_info(path: &str) -> Option<WorktreeInfo> {
    let repo = Repository::discover(path).ok()?;
    let is_worktree = repo.is_worktree();

    // Get the main repo path
    let main_repo_path = if is_worktree {
        // For a worktree, repo.path() is something like /main-repo/.git/worktrees/<name>/
        // Navigate up to find the .git dir, then its parent is the main checkout
        let git_path = repo.path(); // e.g. /main-repo/.git/worktrees/<name>/
        let git_dir = git_path
            .ancestors()
            .find(|p| p.file_name().is_some_and(|n| n == ".git"))?;
        git_dir.parent()?.to_path_buf()
    } else {
        repo.workdir()?.to_path_buf()
    };

    // Normalize: strip trailing slash for consistency
    let main_repo_path = main_repo_path
        .canonicalize()
        .unwrap_or(main_repo_path);

    let main_repo = Repository::open(&main_repo_path).ok()?;
    let repo_name: String = main_repo_path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".to_string());

    let mut entries = Vec::new();

    // Add the main checkout itself
    let main_branch = main_repo
        .head()
        .ok()
        .and_then(|h| h.shorthand().map(|s| s.to_string()));
    entries.push(WorktreeEntry {
        name: repo_name.clone(),
        path: main_repo_path.to_string_lossy().to_string(),
        branch: main_branch,
        is_main: true,
    });

    // List all linked worktrees
    if let Ok(worktrees) = main_repo.worktrees() {
        for i in 0..worktrees.len() {
            if let Some(wt_name) = worktrees.get(i) {
                if let Ok(wt) = main_repo.find_worktree(wt_name) {
                    let wt_path = wt.path();
                    if !wt_path.exists() {
                        continue; // Skip worktrees whose directories were deleted
                    }
                    let wt_path_str = wt_path.to_string_lossy().to_string();

                    // Get branch for this worktree
                    let branch = Repository::open(wt_path).ok().and_then(|r| {
                        r.head()
                            .ok()
                            .and_then(|h| h.shorthand().map(|s| s.to_string()))
                    });

                    entries.push(WorktreeEntry {
                        name: wt_name.to_string(),
                        path: wt_path_str,
                        branch,
                        is_main: false,
                    });
                }
            }
        }
    }

    // If only the main checkout exists (no linked worktrees), it's standalone
    if entries.len() <= 1 {
        return None;
    }

    Some(WorktreeInfo {
        is_worktree,
        repo_name,
        entries,
    })
}

/// Detect the foreground process of a PTY child.
/// Reads /proc/{pid}/stat to find the foreground process group,
/// then /proc/{fg_pid}/cmdline for the process name.
#[derive(Debug, Clone, Serialize)]
pub struct ProcessInfo {
    pub name: String,
    pub is_agent: bool,
    pub agent_label: Option<String>,
}

pub fn get_foreground_process(child_pid: u32) -> Option<ProcessInfo> {
    // Read child's stat to get the terminal's foreground process group
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", child_pid)).ok()?;
    // stat format: pid (comm) state ppid pgrp session tpgid ...
    // tpgid is field 7 (0-indexed after splitting)
    // Find the closing paren to handle comm with spaces
    let comm_end = stat.find(')')?;
    let after_comm = &stat[comm_end + 2..]; // skip ") "
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    // fields[0]=state, [1]=ppid, [2]=pgrp, [3]=session, [4]=tpgid
    let tpgid: i32 = fields.get(4)?.parse().ok()?;

    if tpgid <= 0 {
        return None;
    }

    // Read the foreground process's cmdline
    let cmdline =
        std::fs::read_to_string(format!("/proc/{}/cmdline", tpgid)).ok()?;
    let exe_name = cmdline
        .split('\0')
        .next()?
        .rsplit('/')
        .next()?
        .to_string();

    let (is_agent, agent_label) = match exe_name.as_str() {
        "claude" => (true, Some("Claude Code".to_string())),
        "codex" => (true, Some("Codex".to_string())),
        "aider" => (true, Some("Aider".to_string())),
        "opencode" => (true, Some("OpenCode".to_string())),
        _ => (false, None),
    };

    Some(ProcessInfo {
        name: exe_name,
        is_agent,
        agent_label,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;
    use tempfile::tempdir;

    fn init_test_repo() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        // Create initial commit
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            let file_path = dir.path().join("README.md");
            fs::write(&file_path, "hello").unwrap();
            index.add_path(std::path::Path::new("README.md")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        dir
    }

    #[test]
    fn test_git_info_clean_repo() {
        let dir = init_test_repo();
        let info = git_info_for_path(dir.path().to_str().unwrap());
        assert!(!info.is_dirty);
        assert!(info.branch.is_some());
    }

    #[test]
    fn test_git_info_dirty_repo() {
        let dir = init_test_repo();
        // Modify a tracked file
        fs::write(dir.path().join("README.md"), "modified").unwrap();
        let info = git_info_for_path(dir.path().to_str().unwrap());
        assert!(info.is_dirty);
    }

    #[test]
    fn test_git_info_branch_name() {
        let dir = init_test_repo();
        let repo = Repository::open(dir.path()).unwrap();

        // Create and checkout a new branch
        let head = repo.head().unwrap().peel_to_commit().unwrap();
        repo.branch("feature/test", &head, false).unwrap();
        repo.set_head("refs/heads/feature/test").unwrap();

        let info = git_info_for_path(dir.path().to_str().unwrap());
        assert_eq!(info.branch.as_deref(), Some("feature/test"));
    }

    #[test]
    fn test_git_info_detached_head() {
        let dir = init_test_repo();
        let repo = Repository::open(dir.path()).unwrap();
        let head_oid = repo.head().unwrap().target().unwrap();
        repo.set_head_detached(head_oid).unwrap();

        let info = git_info_for_path(dir.path().to_str().unwrap());
        assert!(info.detached);
        assert!(info.branch.is_some()); // shows short SHA
    }

    #[test]
    fn test_git_info_non_git_dir() {
        let dir = tempdir().unwrap();
        let info = git_info_for_path(dir.path().to_str().unwrap());
        assert!(info.branch.is_none());
        assert!(!info.is_dirty);
    }

    #[test]
    fn test_git_info_deleted_dir() {
        let info = git_info_for_path("/nonexistent/path/12345");
        assert!(info.branch.is_none());
        assert!(!info.is_dirty);
    }

    #[test]
    fn test_detect_worktree_standalone_repo() {
        let dir = init_test_repo();
        let result = detect_worktree_info(dir.path().to_str().unwrap());
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_worktree_non_git_dir() {
        let dir = tempdir().unwrap();
        let result = detect_worktree_info(dir.path().to_str().unwrap());
        assert!(result.is_none());
    }

    /// Helper: create a worktree in a sibling directory of the given repo.
    /// Returns the worktree path (wrapped in a TempDir would auto-clean, but
    /// since the repo's tempdir owns the worktree metadata, manual cleanup isn't needed).
    fn add_worktree(repo_dir: &std::path::Path, name: &str, branch: &str) -> std::path::PathBuf {
        let wt_path = repo_dir.join(name);
        let status = Command::new("git")
            .args(["worktree", "add", wt_path.to_str().unwrap(), "-b", branch])
            .current_dir(repo_dir)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "git worktree add failed: {}",
            String::from_utf8_lossy(&status.stderr)
        );
        wt_path
    }

    /// Creates a temp directory with an initialized git repo inside a "main" subdirectory.
    /// Returns (outer_tempdir, main_repo_path) so worktrees can be created as siblings.
    fn init_worktree_test_repo() -> (tempfile::TempDir, std::path::PathBuf) {
        let outer = tempdir().unwrap();
        let main_path = outer.path().join("main-repo");
        fs::create_dir_all(&main_path).unwrap();
        let repo = Repository::init(&main_path).unwrap();

        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            let file_path = main_path.join("README.md");
            fs::write(&file_path, "hello").unwrap();
            index.add_path(std::path::Path::new("README.md")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "initial", &tree, &[])
            .unwrap();

        (outer, main_path)
    }

    #[test]
    fn test_detect_worktree_from_main_checkout() {
        let (_outer, main_path) = init_worktree_test_repo();
        let wt_path = add_worktree(&main_path, "../wt-feat", "feat");

        let info = detect_worktree_info(main_path.to_str().unwrap()).unwrap();
        assert!(!info.is_worktree); // We're querying from the main checkout
        assert_eq!(info.entries.len(), 2);
        let main_entry = info.entries.iter().find(|e| e.is_main).unwrap();
        // Canonicalize for comparison
        let main_canon = main_path.canonicalize().unwrap();
        assert_eq!(main_entry.path, main_canon.to_str().unwrap());
        let wt_entry = info.entries.iter().find(|e| !e.is_main).unwrap();
        assert_eq!(wt_entry.branch.as_deref(), Some("feat"));
        // Cleanup worktree dir (owned by outer tempdir, but be explicit)
        drop(wt_path);
    }

    #[test]
    fn test_detect_worktree_from_worktree_path() {
        let (_outer, main_path) = init_worktree_test_repo();
        let wt_path = add_worktree(&main_path, "../wt-feat2", "feat2");

        let info = detect_worktree_info(wt_path.to_str().unwrap()).unwrap();
        assert!(info.is_worktree); // We're querying from a linked worktree
        assert_eq!(info.entries.len(), 2);
        assert!(info.entries.iter().any(|e| e.is_main));
        assert!(info.entries.iter().any(|e| !e.is_main));
    }

    #[test]
    fn test_detect_worktree_multiple() {
        let (_outer, main_path) = init_worktree_test_repo();

        for (name, branch) in &[("../wt-a", "feat-a"), ("../wt-b", "feat-b"), ("../wt-c", "feat-c")] {
            add_worktree(&main_path, name, branch);
        }

        let info = detect_worktree_info(main_path.to_str().unwrap()).unwrap();
        assert_eq!(info.entries.len(), 4); // main + 3 worktrees
    }

    #[test]
    fn test_detect_worktree_deleted_worktree_dir() {
        let (_outer, main_path) = init_worktree_test_repo();
        let wt_del = add_worktree(&main_path, "../wt-del", "del-branch");
        let _wt_keep = add_worktree(&main_path, "../wt-keep", "keep-branch");

        // Delete the worktree directory (simulates user deleting it)
        fs::remove_dir_all(&wt_del).unwrap();

        let info = detect_worktree_info(main_path.to_str().unwrap()).unwrap();
        // Should still return info but skip the deleted worktree
        assert!(info.entries.len() >= 2); // main + at least the kept worktree
        let wt_del_canon = wt_del.to_str().unwrap();
        assert!(!info.entries.iter().any(|e| e.path == wt_del_canon));
    }

    #[test]
    fn test_detect_worktree_branch_names() {
        let (_outer, main_path) = init_worktree_test_repo();
        add_worktree(&main_path, "../wt-branch-test", "feature/auth");

        let info = detect_worktree_info(main_path.to_str().unwrap()).unwrap();
        let wt_entry = info.entries.iter().find(|e| !e.is_main).unwrap();
        assert_eq!(wt_entry.branch.as_deref(), Some("feature/auth"));
    }

    #[test]
    fn test_git_info_ahead_behind() {
        let dir = init_test_repo();
        let repo = Repository::open(dir.path()).unwrap();

        let head_ref = repo.head().unwrap();
        let branch_name = head_ref.shorthand().unwrap_or("master");
        let head_oid = head_ref.target().unwrap();

        // Create a fake remote tracking ref pointing to initial commit
        repo.reference(
            &format!("refs/remotes/origin/{}", branch_name),
            head_oid,
            true,
            "test setup",
        )
        .unwrap();

        // Now make a local commit to be ahead by 1
        let sig = git2::Signature::now("Test", "test@test.com").unwrap();
        let file_path = dir.path().join("new_file.txt");
        fs::write(&file_path, "new content").unwrap();
        let mut index = repo.index().unwrap();
        index
            .add_path(std::path::Path::new("new_file.txt"))
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo.head().unwrap().peel_to_commit().unwrap();
        repo.commit(Some("HEAD"), &sig, &sig, "second commit", &tree, &[&parent])
            .unwrap();

        let info = git_info_for_path(dir.path().to_str().unwrap());
        assert_eq!(info.ahead, 1);
        assert_eq!(info.behind, 0);
    }
}
