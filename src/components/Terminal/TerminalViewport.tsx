import { useState, useCallback } from "react";
import { useAppStore } from "../../stores/appStore";
import { TerminalInstance } from "./TerminalInstance";
import { EmptyState } from "./EmptyState";
import { terminalManager } from "../../lib/terminalManager";
import { terminalClose, terminalCreate, terminalSetActive } from "../../lib/tauriCommands";
import { SearchBar } from "./SearchBar";
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
    async (tabId: string, repoId: string) => {
      // Destroy JS terminal
      terminalManager.destroy(tabId);
      setExitedTabs((prev) => {
        const next = new Map(prev);
        next.delete(tabId);
        return next;
      });
      // Close the Rust PTY session + DB record
      try {
        await terminalClose(tabId);
      } catch {
        // May already be closed
      }
      // Create a fresh session
      const newSession: any = await terminalCreate(repoId);
      const newId = newSession.id;
      await terminalSetActive(repoId, newId);
      // Update the store: replace the old tab with the new one
      useAppStore.setState((s) => {
        const repo = s.repos.find((r) => r.id === repoId);
        if (repo) {
          const idx = repo.tabs.findIndex((t) => t.id === tabId);
          const newTab = {
            id: newId,
            repoId: newSession.repo_id ?? repoId,
            name: newSession.title ?? `Terminal`,
            shell: newSession.shell ?? null,
            sortOrder: newSession.sort_order ?? 0,
          };
          if (idx >= 0) {
            repo.tabs[idx] = newTab;
          } else {
            repo.tabs.push(newTab);
          }
          repo.activeTabId = newId;
        }
      });
    },
    []
  );

  if (!activeRepo || repos.length === 0) {
    return <EmptyState />;
  }

  return (
    <div className={styles.viewport}>
      <SearchBar />
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
                    onClick={() => handleRestart(tab.id, repo.id)}
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
