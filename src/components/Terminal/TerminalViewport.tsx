import { useState, useCallback } from "react";
import { useAppStore } from "../../stores/appStore";
import { TerminalInstance } from "./TerminalInstance";
import { EmptyState } from "./EmptyState";
import { terminalManager } from "../../lib/terminalManager";
import styles from "./TerminalViewport.module.css";

export function TerminalViewport() {
  const repos = useAppStore((s) => s.repos);
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const [exitedTabs, setExitedTabs] = useState<
    Map<string, number | null>
  >(new Map());

  const activeRepo = repos.find((r) => r.id === activeRepoId);

  const handleExit = useCallback(
    (tabId: string, code: number | null) => {
      setExitedTabs((prev) => new Map(prev).set(tabId, code));
    },
    []
  );

  const handleRestart = useCallback(
    async (tabId: string) => {
      // Destroy the old terminal and remove exit status
      terminalManager.destroy(tabId);
      setExitedTabs((prev) => {
        const next = new Map(prev);
        next.delete(tabId);
        return next;
      });
      // The TerminalInstance effect will re-create it
    },
    []
  );

  if (!activeRepo || repos.length === 0) {
    return <EmptyState />;
  }

  return (
    <div className={styles.viewport}>
      {repos.flatMap((repo) =>
        repo.tabs.map((tab) => {
          const isVisible =
            repo.id === activeRepoId && tab.id === repo.activeTabId;
          const exitCode = exitedTabs.get(tab.id);
          const hasExited = exitedTabs.has(tab.id);

          return (
            <div
              key={tab.id}
              className={`${styles.terminalWrapper} ${!isVisible ? styles.hidden : ""}`}
            >
              <TerminalInstance
                tabId={tab.id}
                isVisible={isVisible}
                onExit={(code) => handleExit(tab.id, code)}
              />
              {hasExited && isVisible && (
                <div className={styles.exitOverlay}>
                  <span>
                    Process {exitCode !== null ? `exited (code ${exitCode})` : "terminated"}
                  </span>
                  <button
                    className={styles.restartButton}
                    onClick={() => handleRestart(tab.id)}
                  >
                    Restart
                  </button>
                </div>
              )}
            </div>
          );
        })
      )}
    </div>
  );
}
