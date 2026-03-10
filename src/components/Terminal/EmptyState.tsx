import styles from "./TerminalViewport.module.css";

export function EmptyState() {
  return (
    <div className={styles.viewport} style={{ display: "flex", alignItems: "center", justifyContent: "center" }}>
      <div style={{ textAlign: "center", color: "var(--text-tertiary)" }}>
        <p style={{ fontSize: "var(--font-size-base)", marginBottom: "var(--space-2)" }}>
          No repositories yet
        </p>
        <p style={{ fontSize: "var(--font-size-sm)" }}>
          Click "Add repository" in the sidebar to get started
        </p>
      </div>
    </div>
  );
}
