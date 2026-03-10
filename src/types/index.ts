export interface Repo {
  id: string;
  path: string;
  name: string;
  sortOrder: number;
  groupId: string | null;
  isDefault: boolean;
}

export interface TerminalTab {
  id: string;
  repoId: string;
  name: string;
  shell: string | null;
  sortOrder: number;
}

export interface GitInfo {
  branch: string | null;
  is_dirty: boolean;
  detached: boolean;
  ahead: number;
  behind: number;
}

export interface RepoWithState extends Repo {
  gitInfo: GitInfo;
  tabs: TerminalTab[];
  activeTabId: string | null;
}

export interface AppLayout {
  window_x: number | null;
  window_y: number | null;
  window_width: number;
  window_height: number;
  window_maximized: boolean;
  sidebar_width: number;
  sidebar_collapsed: boolean;
  active_repo_id: string | null;
  collapsed_group_ids: string[];
}

export interface UserConfig {
  default_shell: string;
  font_family: string;
  font_size: number;
  scrollback_lines: number;
  theme: string;
  git_poll_interval_secs: number;
  ui_scale: number;
}

export interface ProcessInfo {
  name: string;
  is_agent: boolean;
  agent_label: string | null;
}

export interface WorktreeEntry {
  name: string;
  path: string;
  branch: string | null;
  is_main: boolean;
}

export interface WorktreeInfo {
  is_worktree: boolean;
  repo_name: string;
  entries: WorktreeEntry[];
}

export interface RepoGroup {
  groupId: string;
  name: string;
  repos: RepoWithState[];
  isWorktreeGroup: boolean;
}

export type TerminalOutputData =
  | { kind: "Data"; data: string }
  | { kind: "Exited"; code: number | null };
