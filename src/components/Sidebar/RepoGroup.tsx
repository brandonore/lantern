import { useAppStore } from "../../stores/appStore";
import { RepoItem } from "./RepoItem";
import type { RepoGroup as RepoGroupType } from "../../types";
import styles from "./RepoGroup.module.css";

interface Props {
  group: RepoGroupType;
}

export function RepoGroup({ group }: Props) {
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const setActiveRepo = useAppStore((s) => s.setActiveRepo);
  const removeRepo = useAppStore((s) => s.removeRepo);
  const collapsed = useAppStore((s) => s.collapsedGroupIds.includes(group.groupId));
  const toggleGroupCollapsed = useAppStore((s) => s.toggleGroupCollapsed);

  return (
    <div className={styles.group}>
      <button
        className={styles.groupHeader}
        onClick={() => toggleGroupCollapsed(group.groupId)}
        type="button"
      >
        <span className={`${styles.chevron} ${collapsed ? styles.chevronCollapsed : ""}`}>›</span>
        {group.name}
      </button>
      {!collapsed &&
        group.repos.map((repo) => (
          <RepoItem
            key={repo.id}
            repo={repo}
            isActive={repo.id === activeRepoId}
            isGrouped={group.isWorktreeGroup}
            onClick={() => setActiveRepo(repo.id)}
            onRemove={() => removeRepo(repo.id)}
          />
        ))}
    </div>
  );
}
