import { useRef, useEffect, useCallback } from "react";
import { useAppStore } from "../../stores/appStore";
import { terminalManager } from "../../lib/terminalManager";
import styles from "./TerminalViewport.module.css";

export function SearchBar() {
  const searchOpen = useAppStore((s) => s.searchOpen);
  const searchQuery = useAppStore((s) => s.searchQuery);
  const setSearchQuery = useAppStore((s) => s.setSearchQuery);
  const setSearchOpen = useAppStore((s) => s.setSearchOpen);
  const repos = useAppStore((s) => s.repos);
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const inputRef = useRef<HTMLInputElement>(null);

  const activeRepo = repos.find((r) => r.id === activeRepoId);
  const activeTabId = activeRepo?.activeTabId;

  useEffect(() => {
    if (searchOpen && inputRef.current) {
      inputRef.current.focus();
      inputRef.current.select();
    }
  }, [searchOpen]);

  const handleChange = useCallback(
    (value: string) => {
      setSearchQuery(value);
      if (activeTabId && value) {
        terminalManager.search(activeTabId, value);
      }
    },
    [activeTabId, setSearchQuery]
  );

  const handleKeyDown = useCallback(
    (e: React.KeyboardEvent) => {
      if (e.key === "Escape") {
        setSearchOpen(false);
      } else if (e.key === "Enter") {
        if (activeTabId && searchQuery) {
          terminalManager.search(activeTabId, searchQuery);
        }
      }
    },
    [activeTabId, searchQuery, setSearchOpen]
  );

  if (!searchOpen) return null;

  return (
    <div className={styles.searchBar}>
      <input
        ref={inputRef}
        className={styles.searchInput}
        type="text"
        placeholder="Search..."
        value={searchQuery}
        onChange={(e) => handleChange(e.target.value)}
        onKeyDown={handleKeyDown}
      />
      <button
        className={styles.searchClose}
        onClick={() => setSearchOpen(false)}
        title="Close (Escape)"
      >
        x
      </button>
    </div>
  );
}
