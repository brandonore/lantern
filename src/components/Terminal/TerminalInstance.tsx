import { useRef, useEffect } from "react";
import { terminalManager } from "../../lib/terminalManager";
import { useAppStore } from "../../stores/appStore";

interface Props {
  tabId: string;
  isVisible: boolean;
  onExit?: (code: number | null) => void;
}

export function TerminalInstance({ tabId, isVisible, onExit }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const config = useAppStore((s) => s.config);

  useEffect(() => {
    if (!containerRef.current || !config) return;
    if (terminalManager.has(tabId)) return;

    terminalManager.create(
      tabId,
      containerRef.current,
      {
        fontFamily: config.font_family,
        fontSize: config.font_size,
        scrollback: config.scrollback_lines,
      },
      onExit
    );

    return () => {
      // Don't destroy on unmount — we keep terminals alive
    };
  }, [tabId, config]);

  useEffect(() => {
    if (isVisible) {
      terminalManager.fitAndFocus(tabId);
    }
  }, [isVisible, tabId]);

  return (
    <div
      ref={containerRef}
      style={{
        width: "100%",
        height: "100%",
        display: isVisible ? "block" : "none",
      }}
    />
  );
}
