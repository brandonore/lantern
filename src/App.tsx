import { useEffect } from "react";
import { AppShell } from "./components/AppShell";
import { useAppStore } from "./stores/appStore";
import { useGitPoller } from "./hooks/useGitPoller";
import { useShortcuts } from "./hooks/useShortcuts";

export function App() {
  const hydrate = useAppStore((s) => s.hydrate);

  useEffect(() => {
    hydrate();
  }, [hydrate]);

  useGitPoller();
  useShortcuts();

  return <AppShell />;
}
