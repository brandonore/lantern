import { useEffect, useState } from "react";
import { terminalManager } from "../lib/terminalManager";
import type { ProcessInfo } from "../types";

export function useAgentDetector(): ProcessInfo | null {
  const [processInfo, setProcessInfo] = useState<ProcessInfo | null>(() =>
    terminalManager.getActiveProcessInfo()
  );

  useEffect(() => {
    return terminalManager.subscribeActiveProcess(setProcessInfo);
  }, []);

  return processInfo;
}
