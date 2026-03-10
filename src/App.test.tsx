import { beforeEach, describe, expect, it, vi } from "vitest";
import { act, render, waitFor } from "@testing-library/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "./stores/appStore";

vi.mock("./components/AppShell", () => ({
  AppShell: () => <div>App Shell</div>,
}));

vi.mock("./hooks/useGitPoller", () => ({
  useGitPoller: vi.fn(),
}));

vi.mock("./hooks/useShortcuts", () => ({
  useShortcuts: vi.fn(),
}));

vi.mock("./lib/tauriCommands", () => ({
  stateSaveLayout: vi.fn().mockResolvedValue(undefined),
}));

import { stateSaveLayout } from "./lib/tauriCommands";
import { App } from "./App";

const mockStateSaveLayout = stateSaveLayout as ReturnType<typeof vi.fn>;

describe("App", () => {
  beforeEach(() => {
    mockStateSaveLayout.mockReset();
    mockStateSaveLayout.mockResolvedValue(undefined);

    const appWindow = getCurrentWindow() as any;
    appWindow.scaleFactor.mockResolvedValue(1);
    appWindow.outerSize.mockResolvedValue({ width: 1440, height: 900 });
    appWindow.outerPosition.mockResolvedValue({ x: 20, y: 30 });
    appWindow.isMaximized.mockResolvedValue(false);

    useAppStore.setState({
      hydrate: vi.fn().mockResolvedValue(undefined),
      sidebarWidth: 320,
      sidebarCollapsed: true,
      activeRepoId: "repo-1",
      collapsedGroupIds: ["g1"],
    });
  });

  it("saves the collapsed sidebar state when the window closes", async () => {
    const appWindow = getCurrentWindow() as any;
    let onCloseRequested: (() => Promise<void>) | undefined;

    appWindow.onCloseRequested.mockImplementation((callback: () => Promise<void>) => {
      onCloseRequested = callback;
      return Promise.resolve(() => {});
    });

    render(<App />);

    await waitFor(() => expect(appWindow.onCloseRequested).toHaveBeenCalledTimes(1));

    await act(async () => {
      await onCloseRequested?.();
    });

    expect(mockStateSaveLayout).toHaveBeenCalledWith({
      window_x: 20,
      window_y: 30,
      window_width: 1440,
      window_height: 900,
      window_maximized: false,
      sidebar_width: 320,
      sidebar_collapsed: true,
      active_repo_id: "repo-1",
      collapsed_group_ids: ["g1"],
    });
  });
});
