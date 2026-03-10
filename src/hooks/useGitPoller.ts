import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { useAppStore } from "../stores/appStore";
import type { GitInfo } from "../types";

export function useGitPoller() {
  const updateGitStatus = useAppStore((s) => s.updateGitStatus);

  useEffect(() => {
    const unlisten = listen<[string, GitInfo][]>(
      "git-status-update",
      (event) => {
        updateGitStatus(event.payload);
      }
    );

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [updateGitStatus]);
}
