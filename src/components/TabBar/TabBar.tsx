import { useAppStore } from "../../stores/appStore";
import { Tab } from "./Tab";
import styles from "./TabBar.module.css";

export function TabBar() {
  const repos = useAppStore((s) => s.repos);
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const addTab = useAppStore((s) => s.addTab);
  const closeTab = useAppStore((s) => s.closeTab);
  const setActiveTab = useAppStore((s) => s.setActiveTab);
  const renameTab = useAppStore((s) => s.renameTab);

  const activeRepo = repos.find((r) => r.id === activeRepoId);
  if (!activeRepo) return null;

  return (
    <div className={styles.tabBar}>
      {activeRepo.tabs.map((tab) => (
        <Tab
          key={tab.id}
          tab={tab}
          isActive={tab.id === activeRepo.activeTabId}
          onClick={() => setActiveTab(activeRepo.id, tab.id)}
          onClose={() => closeTab(activeRepo.id, tab.id)}
          onRename={(name) => renameTab(tab.id, name)}
        />
      ))}
      <button
        className={styles.newTabButton}
        onClick={() => addTab(activeRepo.id)}
        title="New terminal (Ctrl+T)"
      >
        +
      </button>
    </div>
  );
}
