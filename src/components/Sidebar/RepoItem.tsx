import type { RepoWithState } from "../../types";
import styles from "./RepoItem.module.css";

interface Props {
  repo: RepoWithState;
  isActive: boolean;
  isGrouped?: boolean;
  onClick: () => void;
  onRemove: () => void;
}

function StarIcon() {
  return (
    <svg className={styles.icon} viewBox="0 0 16 16" fill="currentColor">
      <path d="M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279l-3.046 2.97.719 4.192a.75.75 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25z" />
    </svg>
  );
}

function BranchIcon() {
  return (
    <svg className={styles.icon} viewBox="0 0 16 16" fill="currentColor">
      <path fillRule="evenodd" d="M11.75 2.5a.75.75 0 1 0 0 1.5.75.75 0 0 0 0-1.5zm-2.25.75a2.25 2.25 0 1 1 3 2.122V6c0 .73-.593 1.322-1.325 1.322H8.822A1.325 1.325 0 0 0 7.5 8.647v1.231a2.251 2.251 0 1 1-1.5 0V5.372a2.25 2.25 0 1 1 1.5 0v1.836A2.82 2.82 0 0 1 8.822 6.75h2.353c.085 0 .175-.091.175-.178v-.2A2.251 2.251 0 0 1 9.5 3.25zM6 3.25a.75.75 0 1 0-1.5 0 .75.75 0 0 0 1.5 0zM6 12.75a.75.75 0 1 0-1.5 0 .75.75 0 0 0 1.5 0z" />
    </svg>
  );
}

function RepoIcon({ isGrouped, isDefault }: { isGrouped: boolean; isDefault: boolean }) {
  if (!isGrouped || isDefault) {
    return <StarIcon />;
  }
  return <BranchIcon />;
}

export function RepoItem({ repo, isActive, isGrouped = false, onClick, onRemove }: Props) {
  const classNames = [
    styles.repoItem,
    isActive ? styles.active : "",
  ]
    .filter(Boolean)
    .join(" ");

  const { gitInfo } = repo;

  return (
    <div className={classNames} onClick={onClick}>
      <RepoIcon isGrouped={isGrouped} isDefault={repo.isDefault} />
      <div className={styles.info}>
        <div className={styles.nameRow}>
          <span className={styles.name}>{repo.name}</span>
          <div className={styles.badges}>
            {isGrouped && repo.isDefault && (
              <span className={styles.defaultBadge}>default</span>
            )}
            {gitInfo.is_dirty && (
              <span className={styles.dirtyBadge}>M</span>
            )}
          </div>
        </div>
        <div className={styles.meta}>
          {gitInfo.branch && (
            <span className={styles.branch}>{gitInfo.branch}</span>
          )}
          {gitInfo.ahead > 0 && (
            <span className={styles.ahead}>↑{gitInfo.ahead}</span>
          )}
          {gitInfo.behind > 0 && (
            <span className={styles.behind}>↓{gitInfo.behind}</span>
          )}
        </div>
      </div>
      <button
        className={styles.removeButton}
        onClick={(e) => {
          e.stopPropagation();
          onRemove();
        }}
        title="Remove repository"
      >
        x
      </button>
    </div>
  );
}
