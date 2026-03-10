import { useState, useEffect } from "react";
import { useAppStore } from "../../stores/appStore";
import { configUpdate } from "../../lib/tauriCommands";
import type { UserConfig } from "../../types";
import styles from "./SettingsDialog.module.css";

export function SettingsDialog() {
  const config = useAppStore((s) => s.config);
  const setConfig = useAppStore((s) => s.setConfig);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const [draft, setDraft] = useState<UserConfig | null>(null);

  useEffect(() => {
    if (config) setDraft({ ...config });
  }, [config]);

  if (!draft) return null;

  const handleSave = async () => {
    try {
      const updated = await configUpdate(draft);
      setConfig(updated);
      setSettingsOpen(false);
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  };

  const handleCancel = () => {
    setSettingsOpen(false);
  };

  return (
    <div className={styles.overlay} onClick={handleCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h2 className={styles.title}>Settings</h2>

        <div className={styles.field}>
          <label className={styles.label}>Default Shell</label>
          <input
            className={styles.input}
            value={draft.default_shell}
            onChange={(e) =>
              setDraft({ ...draft, default_shell: e.target.value })
            }
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Font Family</label>
          <input
            className={styles.input}
            value={draft.font_family}
            onChange={(e) =>
              setDraft({ ...draft, font_family: e.target.value })
            }
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Font Size</label>
          <input
            className={styles.input}
            type="number"
            min={8}
            max={32}
            value={draft.font_size}
            onChange={(e) =>
              setDraft({ ...draft, font_size: parseInt(e.target.value) || 14 })
            }
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Scrollback Lines</label>
          <input
            className={styles.input}
            type="number"
            min={100}
            max={100000}
            value={draft.scrollback_lines}
            onChange={(e) =>
              setDraft({
                ...draft,
                scrollback_lines: parseInt(e.target.value) || 10000,
              })
            }
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label}>Git Poll Interval (seconds)</label>
          <input
            className={styles.input}
            type="number"
            min={1}
            max={60}
            value={draft.git_poll_interval_secs}
            onChange={(e) =>
              setDraft({
                ...draft,
                git_poll_interval_secs: parseInt(e.target.value) || 5,
              })
            }
          />
        </div>

        <div className={styles.actions}>
          <button
            className={`${styles.button} ${styles.buttonSecondary}`}
            onClick={handleCancel}
          >
            Cancel
          </button>
          <button
            className={`${styles.button} ${styles.buttonPrimary}`}
            onClick={handleSave}
          >
            Save
          </button>
        </div>
      </div>
    </div>
  );
}
