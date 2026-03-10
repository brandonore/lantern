import { useAppStore } from "../../stores/appStore";
import { useAgentDetector } from "../../hooks/useAgentDetector";
import { terminalManager } from "../../lib/terminalManager";
import styles from "./StatusBar.module.css";

export function StatusBar() {
  const repos = useAppStore((s) => s.repos);
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const processInfo = useAgentDetector();

  const activeRepo = repos.find((r) => r.id === activeRepoId);
  const activeTabId = activeRepo?.activeTabId ?? null;
  const dims = activeTabId ? terminalManager.getDimensions(activeTabId) : undefined;

  return (
    <div className={styles.statusBar}>
      {activeRepo && (
        <span className={styles.item}>{activeRepo.path}</span>
      )}
      {processInfo && (
        <>
          <span className={styles.item}>
            {processInfo.is_agent ? (
              <span className={styles.agent}>{processInfo.agent_label}</span>
            ) : (
              processInfo.name
            )}
          </span>
        </>
      )}
      <span className={styles.spacer} />
      {dims && (
        <span className={styles.item}>
          {dims.cols}x{dims.rows}
        </span>
      )}
    </div>
  );
}
