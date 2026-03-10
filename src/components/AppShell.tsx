import { useAppStore } from "../stores/appStore";
import { useTheme } from "../hooks/useTheme";
import { useUiScale } from "../hooks/useUiScale";
import { TitleBar } from "./TitleBar";
import { Sidebar } from "./Sidebar/Sidebar";
import { TabBar } from "./TabBar/TabBar";
import { TerminalViewport } from "./Terminal/TerminalViewport";
import { StatusBar } from "./StatusBar/StatusBar";
import { SettingsDialog } from "./Settings/SettingsDialog";
import styles from "./AppShell.module.css";

export function AppShell() {
  useTheme();
  useUiScale();
  const sidebarCollapsed = useAppStore((s) => s.sidebarCollapsed);
  const settingsOpen = useAppStore((s) => s.settingsOpen);

  return (
    <div className={styles.shell}>
      <TitleBar />
      <div className={styles.body}>
        <div
          className={`${styles.sidebarArea} ${sidebarCollapsed ? styles.sidebarCollapsed : ""}`}
        >
          <Sidebar />
        </div>
        <div className={styles.mainArea}>
          <TabBar />
          <TerminalViewport />
          <StatusBar />
        </div>
      </div>
      {settingsOpen && <SettingsDialog />}
    </div>
  );
}
