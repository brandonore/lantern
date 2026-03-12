import { useState, useEffect, useRef } from "react";
import { useAppStore } from "../../stores/appStore";
import { configUpdate } from "../../lib/tauriCommands";
import { getAllFamilies, getThemeFamily, getThemeMode, toggleMode, getTheme, applyTheme, applyUiScale } from "../../lib/themes/index";
import { terminalManager } from "../../lib/terminalManager";
import type { UserConfig } from "../../types";
import styles from "./SettingsDialog.module.css";

export function SettingsDialog() {
  const config = useAppStore((s) => s.config);
  const setConfig = useAppStore((s) => s.setConfig);
  const setSettingsOpen = useAppStore((s) => s.setSettingsOpen);
  const [draft, setDraft] = useState<UserConfig | null>(null);
  const originalThemeRef = useRef<string>("");
  const originalFontSizeRef = useRef<number>(14);
  const originalFontFamilyRef = useRef<string>("JetBrains Mono");
  const originalUiScaleRef = useRef<number>(1);
  const originalLatencyModeRef = useRef<UserConfig["terminal_latency_mode"]>("low-latency");

  useEffect(() => {
    if (config) {
      setDraft({ ...config });
      originalThemeRef.current = config.theme;
      originalFontSizeRef.current = config.font_size;
      originalFontFamilyRef.current = config.font_family;
      originalUiScaleRef.current = config.ui_scale ?? 1;
      originalLatencyModeRef.current = config.terminal_latency_mode;
    }
  }, [config]);

  // Live preview: apply theme as draft.theme changes
  useEffect(() => {
    if (!draft) return;
    const variant = getTheme(draft.theme);
    applyTheme(variant);
    terminalManager.updateAllThemes(variant);
  }, [draft?.theme]);

  // Live preview: font size
  useEffect(() => {
    if (!draft) return;
    terminalManager.updateAllFontSize(draft.font_size);
  }, [draft?.font_size]);

  // Live preview: font family
  useEffect(() => {
    if (!draft) return;
    terminalManager.updateAllFontFamily(draft.font_family);
  }, [draft?.font_family]);

  // Live preview: UI scale
  useEffect(() => {
    if (!draft) return;
    applyUiScale(draft.ui_scale ?? 1);
  }, [draft?.ui_scale]);

  useEffect(() => {
    if (!draft) return;
    terminalManager.updateAllLatencyMode(draft.terminal_latency_mode);
  }, [draft?.terminal_latency_mode]);

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
    // Revert theme to original
    const variant = getTheme(originalThemeRef.current);
    applyTheme(variant);
    terminalManager.updateAllThemes(variant);
    // Revert font size/family
    terminalManager.updateAllFontSize(originalFontSizeRef.current);
    terminalManager.updateAllFontFamily(originalFontFamilyRef.current);
    // Revert UI scale
    applyUiScale(originalUiScaleRef.current);
    terminalManager.updateAllLatencyMode(originalLatencyModeRef.current);
    setSettingsOpen(false);
  };

  const families = getAllFamilies();
  const currentFamily = getThemeFamily(draft.theme);
  const currentMode = getThemeMode(draft.theme);
  const uiScalePercent = Math.round((draft.ui_scale ?? 1) * 100);

  return (
    <div className={styles.overlay} onClick={handleCancel}>
      <div className={styles.dialog} onClick={(e) => e.stopPropagation()}>
        <h2 className={styles.title}>Settings</h2>

        <div className={styles.field}>
          <label className={styles.label}>Theme</label>
          <div className={styles.themeRow}>
            <select
              className={styles.select}
              value={`${currentFamily}-${currentMode}`}
              onChange={(e) =>
                setDraft({ ...draft, theme: e.target.value })
              }
            >
              {families.map((family) => (
                <optgroup key={family.id} label={family.name}>
                  <option value={`${family.id}-dark`}>{family.name} Dark</option>
                  <option value={`${family.id}-light`}>{family.name} Light</option>
                </optgroup>
              ))}
            </select>
            <button
              className={styles.modeToggle}
              onClick={() => setDraft({ ...draft, theme: toggleMode(draft.theme) })}
              title={`Switch to ${currentMode === "dark" ? "light" : "dark"} mode`}
            >
              {currentMode === "dark" ? "\u2600" : "\u263E"}
            </button>
          </div>
        </div>

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
          <label className={styles.label}>UI Scale ({uiScalePercent}%)</label>
          <input
            className={styles.range}
            type="range"
            min={0.8}
            max={1.5}
            step={0.05}
            value={draft.ui_scale ?? 1}
            onChange={(e) =>
              setDraft({ ...draft, ui_scale: parseFloat(e.target.value) })
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

        <div className={styles.field}>
          <label className={styles.label}>Terminal Latency</label>
          <select
            className={styles.select}
            value={draft.terminal_latency_mode}
            onChange={(e) =>
              setDraft({
                ...draft,
                terminal_latency_mode: e.target
                  .value as UserConfig["terminal_latency_mode"],
              })
            }
          >
            <option value="low-latency">Low latency</option>
            <option value="compatible">Compatible</option>
          </select>
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
