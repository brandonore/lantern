import { useCallback, useRef } from "react";
import { useAppStore } from "../stores/appStore";

export function useSidebarResize() {
  const setSidebarWidth = useAppStore((s) => s.setSidebarWidth);
  const dragging = useRef(false);

  const onPointerDown = useCallback(
    (e: React.PointerEvent) => {
      e.preventDefault();
      dragging.current = true;
      const startX = e.clientX;
      const startWidth = useAppStore.getState().sidebarWidth;

      const onMove = (e: PointerEvent) => {
        if (!dragging.current) return;
        const delta = e.clientX - startX;
        const newWidth = Math.max(180, Math.min(500, startWidth + delta));
        setSidebarWidth(newWidth);
      };

      const onUp = () => {
        dragging.current = false;
        document.removeEventListener("pointermove", onMove);
        document.removeEventListener("pointerup", onUp);
      };

      document.addEventListener("pointermove", onMove);
      document.addEventListener("pointerup", onUp);
    },
    [setSidebarWidth]
  );

  return { onPointerDown };
}
