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
      <div className={styles.header}>
        <span className={styles.title}>Repositories</span>
      </div>
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
      </div>
      <div className={styles.footer}>
        <button className={styles.addButton} onClick={handleAddRepo}>
          + Add repository
        </button>
      </div>
      <SidebarResizeHandle />
    </div>
  );
}
