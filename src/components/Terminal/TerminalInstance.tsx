import { useRef, useEffect, useState } from "react";
import { terminalManager } from "../../lib/terminalManager";
import { useAppStore } from "../../stores/appStore";
import styles from "./TerminalViewport.module.css";

interface Props {
  tabId: string;
  isVisible: boolean;
  onExit?: (code: number | null) => void;
}

export function TerminalInstance({ tabId, isVisible, onExit }: Props) {
  const containerRef = useRef<HTMLDivElement>(null);
  const config = useAppStore((s) => s.config);
  const [hasOutput, setHasOutput] = useState(() =>
    terminalManager.hasReceivedOutput(tabId)
  );

  useEffect(() => {
    setHasOutput(terminalManager.hasReceivedOutput(tabId));
  }, [tabId]);

  useEffect(() => {
    if (!containerRef.current || !config) return;
    if (terminalManager.has(tabId)) return;
    // Defer creation until the tab is first visible to avoid 0x0 dimensions
    if (!isVisible) return;

    setHasOutput(false);

    terminalManager.create(
      tabId,
      containerRef.current,
      {
        fontFamily: config.font_family,
        fontSize: config.font_size,
        scrollback: config.scrollback_lines,
        latencyMode: config.terminal_latency_mode,
      },
      onExit,
      () => setHasOutput(true)
    );

    return () => {
      // Don't destroy on unmount — we keep terminals alive
    };
  }, [tabId, config, isVisible]);

  useEffect(() => {
    if (isVisible) {
      terminalManager.fitAndFocus(tabId);
    }
  }, [isVisible, tabId]);

  return (
    <div className={styles.instance}>
      <div
        ref={containerRef}
        style={{
          width: "100%",
          height: "100%",
          display: isVisible ? "block" : "none",
        }}
      />
      {isVisible && !hasOutput && (
        <div className={styles.startingOverlay}>Starting shell...</div>
      )}
    </div>
  );
}
