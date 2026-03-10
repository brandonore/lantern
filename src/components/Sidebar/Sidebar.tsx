import { useCallback, useState } from "react";
import { useAppStore } from "../../stores/appStore";
import { useRepoGroups } from "../../hooks/useRepoGroups";
import { RepoGroup } from "./RepoGroup";
import { SidebarResizeHandle } from "./SidebarResizeHandle";
import * as cmd from "../../lib/tauriCommands";
import styles from "./Sidebar.module.css";
import type { WorktreeInfo } from "../../types";

export function Sidebar() {
  const groups = useRepoGroups();
  const addRepo = useAppStore((s) => s.addRepo);
  const addRepoWithWorktrees = useAppStore((s) => s.addRepoWithWorktrees);
  const sidebarWidth = useAppStore((s) => s.sidebarWidth);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const [worktreePrompt, setWorktreePrompt] = useState<{
    info: WorktreeInfo;
    path: string;
  } | null>(null);

  const handleAddRepo = useCallback(async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false });
      if (!selected) return;
      const path = selected as string;

      // Detect worktrees before adding
      const wtInfo = await cmd.repoDetectWorktrees(path);
      if (wtInfo && wtInfo.entries.length > 1) {
        setWorktreePrompt({ info: wtInfo, path });
      } else {
        await addRepo(path);
      }
    } catch (e) {
      console.error("Failed to add repo:", e);
    }
  }, [addRepo]);

  const handleWorktreeAddAll = useCallback(async () => {
    if (!worktreePrompt) return;
    try {
      await addRepoWithWorktrees(worktreePrompt.path);
    } catch (e) {
      console.error("Failed to add worktrees:", e);
    }
    setWorktreePrompt(null);
  }, [worktreePrompt, addRepoWithWorktrees]);

  const handleWorktreeAddOne = useCallback(async () => {
    if (!worktreePrompt) return;
    try {
      await addRepo(worktreePrompt.path);
    } catch (e) {
      console.error("Failed to add repo:", e);
    }
    setWorktreePrompt(null);
  }, [worktreePrompt, addRepo]);

  return (
    <div className={styles.sidebar} style={{ width: sidebarWidth }}>
      <div className={styles.repoList}>
        {groups.map((group) => (
          <RepoGroup key={group.groupId} group={group} />
        ))}
        <button className={styles.addButton} onClick={handleAddRepo}>
          + Add repository
        </button>
      </div>
      <div className={styles.footer}>
        <button
          className={styles.settingsButton}
          onClick={() => setSettingsOpen(true)}
        >
          <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
            <circle cx="12" cy="12" r="3" />
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
          </svg>
          Settings
        </button>
      </div>
      <SidebarResizeHandle />

      {worktreePrompt && (
        <div className={styles.worktreeDialog}>
          <div className={styles.worktreeDialogContent}>
            <p className={styles.worktreeDialogTitle}>
              Worktrees detected for {worktreePrompt.info.repo_name}
            </p>
            <ul className={styles.worktreeList}>
              {worktreePrompt.info.entries.map((entry) => (
                <li key={entry.path}>
                  {entry.branch ?? entry.name}
                  {entry.is_main && " (main)"}
                </li>
              ))}
            </ul>
            <div className={styles.worktreeDialogActions}>
              <button
                className={styles.worktreeDialogBtn}
                onClick={handleWorktreeAddAll}
              >
                Add all
              </button>
              <button
                className={styles.worktreeDialogBtn}
                onClick={handleWorktreeAddOne}
              >
                Add only this one
              </button>
              <button
                className={styles.worktreeDialogBtnCancel}
                onClick={() => setWorktreePrompt(null)}
              >
                Cancel
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}
