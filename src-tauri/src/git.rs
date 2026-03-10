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
