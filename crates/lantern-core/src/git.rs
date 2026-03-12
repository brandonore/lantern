use git2::Repository;
use serde::Serialize;

#[derive(Debug, Clone, Serialize, Default, PartialEq, Eq)]
pub struct GitInfo {
    pub branch: Option<String>,
    pub is_dirty: bool,
    pub detached: bool,
    pub ahead: usize,
    pub behind: usize,
    pub insertions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorktreeEntry {
    pub name: String,
    pub path: String,
    pub branch: Option<String>,
    pub is_main: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct WorktreeInfo {
    pub is_worktree: bool,
    pub repo_name: String,
    pub entries: Vec<WorktreeEntry>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ProcessInfo {
    pub name: String,
    pub is_agent: bool,
    pub agent_label: Option<String>,
}

pub fn git_info_for_path(path: &str) -> GitInfo {
    let repo = match Repository::discover(path) {
        Ok(repo) => repo,
        Err(_) => return GitInfo::default(),
    };

    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => {
            let is_dirty = check_dirty(&repo);
            let (insertions, deletions) = if is_dirty {
                count_diff_stats(&repo)
            } else {
                (0, 0)
            };
            return GitInfo {
                branch: None,
                is_dirty,
                detached: false,
                ahead: 0,
                behind: 0,
                insertions,
                deletions,
            };
        }
    };

    let detached = repo.head_detached().unwrap_or(false);
    let branch = if detached {
        head.target().map(|oid| format!("{oid:.7}"))
    } else {
        head.shorthand().map(str::to_string)
    };

    let is_dirty = check_dirty(&repo);
    let (ahead, behind) = count_ahead_behind(&repo);
    let (insertions, deletions) = if is_dirty {
        count_diff_stats(&repo)
    } else {
        (0, 0)
    };

    GitInfo {
        branch,
        is_dirty,
        detached,
        ahead,
        behind,
        insertions,
        deletions,
    }
}

pub fn detect_worktree_info(path: &str) -> Option<WorktreeInfo> {
    let repo = Repository::discover(path).ok()?;
    let is_worktree = repo.is_worktree();

    let main_repo_path = if is_worktree {
        let git_path = repo.path();
        let git_dir = git_path
            .ancestors()
            .find(|ancestor| ancestor.file_name().is_some_and(|name| name == ".git"))?;
        git_dir.parent()?.to_path_buf()
    } else {
        repo.workdir()?.to_path_buf()
    };

    let main_repo_path = main_repo_path.canonicalize().unwrap_or(main_repo_path);
    let main_repo = Repository::open(&main_repo_path).ok()?;
    let repo_name = main_repo_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "repo".to_string());

    let mut entries = Vec::new();
    let main_branch = main_repo
        .head()
        .ok()
        .and_then(|head| head.shorthand().map(str::to_string));
    entries.push(WorktreeEntry {
        name: repo_name.clone(),
        path: main_repo_path.to_string_lossy().into_owned(),
        branch: main_branch,
        is_main: true,
    });

    if let Ok(worktrees) = main_repo.worktrees() {
        for index in 0..worktrees.len() {
            let Some(worktree_name) = worktrees.get(index) else {
                continue;
            };
            let Ok(worktree) = main_repo.find_worktree(worktree_name) else {
                continue;
            };
            let worktree_path = worktree.path();
            if !worktree_path.exists() {
                continue;
            }

            let branch = Repository::open(worktree_path)
                .ok()
                .and_then(|worktree_repo| {
                    worktree_repo
                        .head()
                        .ok()
                        .and_then(|head| head.shorthand().map(str::to_string))
                });

            entries.push(WorktreeEntry {
                name: worktree_name.to_string(),
                path: worktree_path.to_string_lossy().into_owned(),
                branch,
                is_main: false,
            });
        }
    }

    if entries.len() <= 1 {
        return None;
    }

    Some(WorktreeInfo {
        is_worktree,
        repo_name,
        entries,
    })
}

pub fn get_foreground_process(child_pid: u32) -> Option<ProcessInfo> {
    let stat = std::fs::read_to_string(format!("/proc/{child_pid}/stat")).ok()?;
    let comm_end = stat.find(')')?;
    let after_comm = &stat[comm_end + 2..];
    let fields: Vec<&str> = after_comm.split_whitespace().collect();
    let tpgid: i32 = fields.get(4)?.parse().ok()?;
    if tpgid <= 0 {
        return None;
    }

    let cmdline = std::fs::read_to_string(format!("/proc/{tpgid}/cmdline")).ok()?;
    let exe_name = cmdline.split('\0').next()?.rsplit('/').next()?.to_string();
    Some(classify_process_name(exe_name.as_str()))
}

fn classify_process_name(exe_name: &str) -> ProcessInfo {
    let (is_agent, agent_label) = match exe_name {
        "claude" => (true, Some("Claude Code".to_string())),
        "codex" => (true, Some("Codex".to_string())),
        "aider" => (true, Some("Aider".to_string())),
        "opencode" => (true, Some("OpenCode".to_string())),
        _ => (false, None),
    };

    ProcessInfo {
        name: exe_name.to_string(),
        is_agent,
        agent_label,
    }
}

fn check_dirty(repo: &Repository) -> bool {
    let mut options = git2::StatusOptions::new();
    options
        .include_untracked(true)
        .recurse_untracked_dirs(false)
        .exclude_submodules(true);

    match repo.statuses(Some(&mut options)) {
        Ok(statuses) => !statuses.is_empty(),
        Err(_) => false,
    }
}

fn count_diff_stats(repo: &Repository) -> (usize, usize) {
    let diff = match repo.diff_index_to_workdir(None, None) {
        Ok(diff) => diff,
        Err(_) => return (0, 0),
    };
    match diff.stats() {
        Ok(stats) => (stats.insertions(), stats.deletions()),
        Err(_) => (0, 0),
    }
}

fn count_ahead_behind(repo: &Repository) -> (usize, usize) {
    let head = match repo.head() {
        Ok(head) => head,
        Err(_) => return (0, 0),
    };

    let Some(local_oid) = head.target() else {
        return (0, 0);
    };
    let Some(branch_name) = head.shorthand().map(str::to_string) else {
        return (0, 0);
    };

    let upstream_name = format!("refs/remotes/origin/{branch_name}");
    let upstream_ref = match repo.find_reference(&upstream_name) {
        Ok(reference) => reference,
        Err(_) => return (0, 0),
    };
    let Some(upstream_oid) = upstream_ref.target() else {
        return (0, 0);
    };

    repo.graph_ahead_behind(local_oid, upstream_oid)
        .unwrap_or((0, 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use tempfile::tempdir;

    fn init_test_repo() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        let repo = Repository::init(dir.path()).unwrap();

        let signature = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            let file_path = dir.path().join("README.md");
            fs::write(&file_path, "hello").unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();

        dir
    }

    fn add_worktree(repo_dir: &Path, name: &str, branch: &str) -> PathBuf {
        let worktree_path = repo_dir.join(name);
        let output = Command::new("git")
            .args([
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                branch,
            ])
            .current_dir(repo_dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        worktree_path
    }

    fn init_worktree_test_repo() -> (tempfile::TempDir, PathBuf) {
        let outer = tempdir().unwrap();
        let main_path = outer.path().join("main-repo");
        fs::create_dir_all(&main_path).unwrap();
        let repo = Repository::init(&main_path).unwrap();

        let signature = git2::Signature::now("Test", "test@test.com").unwrap();
        let tree_id = {
            let mut index = repo.index().unwrap();
            let file_path = main_path.join("README.md");
            fs::write(&file_path, "hello").unwrap();
            index.add_path(Path::new("README.md")).unwrap();
            index.write().unwrap();
            index.write_tree().unwrap()
        };
        let tree = repo.find_tree(tree_id).unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
            .unwrap();

        (outer, main_path)
    }

    #[test]
    fn git_info_reports_clean_repo() {
        let dir = init_test_repo();
        let info = git_info_for_path(dir.path().to_str().unwrap());

        assert!(!info.is_dirty);
        assert!(info.branch.is_some());
    }

    #[test]
    fn git_info_reports_dirty_repo() {
        let dir = init_test_repo();
        fs::write(dir.path().join("README.md"), "modified").unwrap();

        let info = git_info_for_path(dir.path().to_str().unwrap());

        assert!(info.is_dirty);
    }

    #[test]
    fn git_info_includes_diff_stats_for_dirty_repo() {
        let dir = init_test_repo();
        fs::write(dir.path().join("README.md"), "modified\nline2\nline3").unwrap();

        let info = git_info_for_path(dir.path().to_str().unwrap());

        assert!(info.is_dirty);
        assert!(info.insertions > 0 || info.deletions > 0);
    }

    #[test]
    fn git_info_has_zero_diff_stats_for_clean_repo() {
        let dir = init_test_repo();

        let info = git_info_for_path(dir.path().to_str().unwrap());

        assert!(!info.is_dirty);
        assert_eq!(info.insertions, 0);
        assert_eq!(info.deletions, 0);
    }

    #[test]
    fn detect_worktree_info_returns_none_for_standalone_repo() {
        let dir = init_test_repo();
        assert!(detect_worktree_info(dir.path().to_str().unwrap()).is_none());
    }

    #[test]
    fn detect_worktree_info_from_main_checkout_lists_linked_worktrees() {
        let (_outer, main_path) = init_worktree_test_repo();
        let worktree_path = add_worktree(&main_path, "../wt-feat", "feat");
        let worktree_path = worktree_path.canonicalize().unwrap();

        let info = detect_worktree_info(main_path.to_str().unwrap()).unwrap();

        assert!(!info.is_worktree);
        assert_eq!(info.entries.len(), 2);
        assert!(info.entries.iter().any(|entry| entry.is_main));
        assert!(info
            .entries
            .iter()
            .any(|entry| entry.path == worktree_path.to_string_lossy()));
    }

    #[test]
    fn detect_worktree_info_from_worktree_marks_it_as_worktree() {
        let (_outer, main_path) = init_worktree_test_repo();
        let worktree_path = add_worktree(&main_path, "../wt-feat2", "feat2");

        let info = detect_worktree_info(worktree_path.to_str().unwrap()).unwrap();

        assert!(info.is_worktree);
        assert_eq!(info.entries.len(), 2);
    }

    #[test]
    fn classify_process_name_marks_known_agents() {
        assert_eq!(
            classify_process_name("claude"),
            ProcessInfo {
                name: "claude".to_string(),
                is_agent: true,
                agent_label: Some("Claude Code".to_string()),
            }
        );
        assert_eq!(
            classify_process_name("opencode"),
            ProcessInfo {
                name: "opencode".to_string(),
                is_agent: true,
                agent_label: Some("OpenCode".to_string()),
            }
        );
    }

    #[test]
    fn classify_process_name_leaves_regular_processes_unlabeled() {
        assert_eq!(
            classify_process_name("bash"),
            ProcessInfo {
                name: "bash".to_string(),
                is_agent: false,
                agent_label: None,
            }
        );
    }
}
