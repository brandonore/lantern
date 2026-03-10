import type { RepoWithState } from "../../types";
import styles from "./RepoItem.module.css";

interface Props {
  repo: RepoWithState;
  isActive: boolean;
  onClick: () => void;
  onRemove: () => void;
}

export function RepoItem({ repo, isActive, onClick, onRemove }: Props) {
  return (
    <div
      className={`${styles.repoItem} ${isActive ? styles.active : ""}`}
      onClick={onClick}
    >
      <div className={styles.info}>
        <span className={styles.name}>{repo.name}</span>
        <div className={styles.meta}>
          {repo.gitInfo.branch && (
            <span className={styles.branch}>{repo.gitInfo.branch}</span>
          )}
          {repo.gitInfo.is_dirty && <span className={styles.dirtyDot} />}
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
