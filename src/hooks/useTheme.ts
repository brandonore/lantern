import { useEffect } from "react";
import { useAppStore } from "../stores/appStore";
import { getTheme, applyTheme } from "../lib/themes/index";
import { terminalManager } from "../lib/terminalManager";

export function useTheme() {
  const themeId = useAppStore((s) => s.config?.theme ?? "nord-dark");

  useEffect(() => {
    const variant = getTheme(themeId);
    applyTheme(variant);
    terminalManager.updateAllThemes(variant);
  }, [themeId]);
}
