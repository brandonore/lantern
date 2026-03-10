import { getCurrentWindow } from "@tauri-apps/api/window";
import styles from "./TitleBar.module.css";

const appWindow = getCurrentWindow();

export function TitleBar() {
  const handleDragStart = (e: React.MouseEvent) => {
    // Only drag on left-click directly on the title bar (not buttons)
    if (e.button === 0) {
      appWindow.startDragging();
    }
  };

  return (
    <div className={styles.titleBar} onMouseDown={handleDragStart}>
      <span className={styles.title}>Lantern</span>
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
