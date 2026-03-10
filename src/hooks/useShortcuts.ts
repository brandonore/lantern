import { useEffect } from "react";
import { useAppStore } from "../stores/appStore";
import { terminalManager } from "../lib/terminalManager";

export function useShortcuts() {
  const store = useAppStore;

  useEffect(() => {
    const handler = (e: KeyboardEvent) => {
      const state = store.getState();
      const activeRepo = state.repos.find(
        (r) => r.id === state.activeRepoId
      );

      // Ctrl+T — new tab
      if (e.ctrlKey && !e.shiftKey && e.key === "t") {
        e.preventDefault();
        if (activeRepo) {
          state.addTab(activeRepo.id);
        }
        return;
      }

      // Ctrl+W — close tab
      if (e.ctrlKey && !e.shiftKey && e.key === "w") {
        e.preventDefault();
        if (activeRepo?.activeTabId) {
          state.closeTab(activeRepo.id, activeRepo.activeTabId);
        }
        return;
      }

      // Ctrl+Tab / Ctrl+Shift+Tab — next/prev tab
      if (e.ctrlKey && e.key === "Tab") {
        e.preventDefault();
        if (!activeRepo || activeRepo.tabs.length === 0) return;
        const idx = activeRepo.tabs.findIndex(
          (t) => t.id === activeRepo.activeTabId
        );
        let newIdx: number;
        if (e.shiftKey) {
          newIdx =
            idx <= 0 ? activeRepo.tabs.length - 1 : idx - 1;
        } else {
          newIdx =
            idx >= activeRepo.tabs.length - 1 ? 0 : idx + 1;
        }
        state.setActiveTab(activeRepo.id, activeRepo.tabs[newIdx].id);
        return;
      }

      // Ctrl+1 through Ctrl+9 — switch to repo N
      if (e.ctrlKey && !e.shiftKey && e.key >= "1" && e.key <= "9") {
        e.preventDefault();
        const idx = parseInt(e.key) - 1;
        if (idx < state.repos.length) {
          state.setActiveRepo(state.repos[idx].id);
        }
        return;
      }

      // Ctrl+B — toggle sidebar
      if (e.ctrlKey && !e.shiftKey && e.key === "b") {
        e.preventDefault();
        state.toggleSidebar();
        return;
      }

      // Escape — focus terminal
      if (e.key === "Escape" && !e.ctrlKey && !e.shiftKey) {
        if (activeRepo?.activeTabId) {
          terminalManager.focus(activeRepo.activeTabId);
        }
        return;
      }

      // Ctrl+, — settings
      if (e.ctrlKey && e.key === ",") {
        e.preventDefault();
        state.setSettingsOpen(!state.settingsOpen);
        return;
      }

      // Ctrl+Shift+F — search in terminal
      if (e.ctrlKey && e.shiftKey && e.key === "F") {
        e.preventDefault();
        // TODO: open search bar
        return;
      }

      // F2 — rename tab
      if (e.key === "F2") {
        e.preventDefault();
        // Handled by Tab component
        return;
      }
    };

    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  }, []);
}
