import { useCallback } from "react";
import { useAppStore } from "../../stores/appStore";
import styles from "./EmptyState.module.css";

export function EmptyState() {
  const addRepo = useAppStore((s) => s.addRepo);

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
    <div className={styles.container}>
      <div className={styles.content}>
        <svg
          className={styles.icon}
          width="32"
          height="32"
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.5"
          strokeLinecap="round"
          strokeLinejoin="round"
        >
          <path d="M4 17l6-6-6-6" />
          <line x1="12" y1="19" x2="20" y2="19" />
        </svg>
        <p className={styles.heading}>No repositories yet</p>
        <p className={styles.subtext}>
          Add a repository to get started
        </p>
        <button className={styles.addButton} onClick={handleAddRepo}>
          + Add repository
        </button>
      </div>
    </div>
  );
}
