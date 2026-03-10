import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppShell } from "./components/AppShell";
import { useAppStore } from "./stores/appStore";
import { useGitPoller } from "./hooks/useGitPoller";
import { useShortcuts } from "./hooks/useShortcuts";
import { stateSaveLayout } from "./lib/tauriCommands";

export function App() {
  const hydrate = useAppStore((s) => s.hydrate);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  // Save layout on window close
  useEffect(() => {
    const unlisten = getCurrentWindow().onCloseRequested(async () => {
      const { sidebarWidth, activeRepoId } = useAppStore.getState();
      const win = getCurrentWindow();
      try {
        const factor = await win.scaleFactor();
        const size = await win.outerSize();
        const pos = await win.outerPosition();
        const maximized = await win.isMaximized();
        await stateSaveLayout({
          window_x: pos.x,
          window_y: pos.y,
          window_width: Math.round(size.width / factor),
          window_height: Math.round(size.height / factor),
          window_maximized: maximized,
          sidebar_width: sidebarWidth,
          active_repo_id: activeRepoId,
        });
      } catch {
        // Best-effort save
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useGitPoller();
  useShortcuts();

  return <AppShell />;
}
