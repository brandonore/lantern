import { useEffect, useState } from "react";
import { useAppStore } from "../stores/appStore";
import { terminalGetForegroundProcess } from "../lib/tauriCommands";
import type { ProcessInfo } from "../types";

export function useAgentDetector(): ProcessInfo | null {
  const [processInfo, setProcessInfo] = useState<ProcessInfo | null>(null);
  const activeRepoId = useAppStore((s) => s.activeRepoId);
  const repos = useAppStore((s) => s.repos);

  const activeRepo = repos.find((r) => r.id === activeRepoId);
  const activeTabId = activeRepo?.activeTabId ?? null;

  useEffect(() => {
    if (!activeTabId) {
      setProcessInfo(null);
      return;
    }

    const poll = async () => {
      try {
        const info = await terminalGetForegroundProcess(activeTabId);
        setProcessInfo(info);
      } catch {
        setProcessInfo(null);
      }
    };

    poll();
    const interval = setInterval(poll, 2000);
    return () => clearInterval(interval);
  }, [activeTabId]);

  return processInfo;
}
