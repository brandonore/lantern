import { useCallback } from "react";
import { useAppStore } from "../../stores/appStore";
import { RepoItem } from "./RepoItem";
import { SidebarResizeHandle } from "./SidebarResizeHandle";
import styles from "./Sidebar.module.css";

export function Sidebar() {
  const repos = useAppStore((s) => s.repos);
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const setActiveRepo = useAppStore((s) => s.setActiveRepo);
  const addRepo = useAppStore((s) => s.addRepo);
  const removeRepo = useAppStore((s) => s.removeRepo);
  const sidebarWidth = useAppStore((s) => s.sidebarWidth);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);

  const handleAddRepo = useCallback(async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const selected = await open({ directory: true, multiple: false });
      if (selected) {
        await addRepo(selected as string);
      }
    } catch (e) {
      console.error("Failed to add repo:", e);
    }
  }, [addRepo]);

  return (
    <div className={styles.sidebar} style={{ width: sidebarWidth }}>
      <div className={styles.repoList}>
        {repos.map((repo) => (
          <RepoItem
            key={repo.id}
            repo={repo}
            isActive={repo.id === activeRepoId}
            onClick={() => setActiveRepo(repo.id)}
            onRemove={() => removeRepo(repo.id)}
          />
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
    </div>
  );
}
