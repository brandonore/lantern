import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "../stores/appStore";
import styles from "./TitleBar.module.css";

const appWindow = getCurrentWindow();

export function TitleBar() {
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const toggleSidebar = useAppStore((s) => s.toggleSidebar);
  const sidebarLabel = sidebarCollapsed ? "Show sidebar" : "Hide sidebar";

  const handleDragStart = (e: React.MouseEvent) => {
    // Only drag on left-click directly on the title bar (not buttons)
    if (e.button === 0) {
      appWindow.startDragging();
    }
  };

  return (
    <div className={styles.titleBar} onMouseDown={handleDragStart}>
      <div className={styles.titleSection}>
        <button
          className={styles.sidebarButton}
          onMouseDown={(e) => e.stopPropagation()}
          onClick={toggleSidebar}
          aria-label={sidebarLabel}
          title={sidebarLabel}
        >
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
            <rect x="1.5" y="2" width="11" height="10" rx="1.5" stroke="currentColor" />
            <path d="M4.5 2.5V11.5" stroke="currentColor" />
            {sidebarCollapsed ? (
              <path d="M6 7H9.5M8 5.5L9.5 7L8 8.5" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" />
            ) : (
              <path d="M9 7H5.5M7 5.5L5.5 7L7 8.5" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" />
            )}
          </svg>
        </button>
        <span className={styles.title}>Lantern</span>
      </div>
      <div
        className={styles.controls}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <button
          className={styles.controlButton}
          onClick={() => appWindow.minimize()}
          aria-label="Minimize"
        >
          <svg width="10" height="1" viewBox="0 0 10 1">
            <rect width="10" height="1" fill="currentColor" />
          </svg>
        </button>
        <button
          className={styles.controlButton}
          onClick={() => appWindow.toggleMaximize()}
          aria-label="Maximize"
        >
          <svg width="9" height="9" viewBox="0 0 9 9" fill="none">
            <rect
              x="0.5"
              y="0.5"
              width="8"
              height="8"
              stroke="currentColor"
              strokeWidth="1"
            />
          </svg>
        </button>
        <button
          className={`${styles.controlButton} ${styles.closeButton}`}
          onClick={() => appWindow.close()}
          aria-label="Close"
        >
          <svg width="10" height="10" viewBox="0 0 10 10">
            <path
              d="M1 1L9 9M9 1L1 9"
              stroke="currentColor"
              strokeWidth="1.2"
            />
          </svg>
        </button>
      </div>
    </div>
  );
}
