import { useEffect } from "react";
import { useAppStore } from "../stores/appStore";
import { applyUiScale } from "../lib/themes/index";

export function useUiScale() {
  const uiScale = useAppStore((s) => s.config?.ui_scale ?? 1);
  useEffect(() => { applyUiScale(uiScale); }, [uiScale]);
}
