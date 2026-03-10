import { describe, it, expect, vi, beforeEach } from "vitest";
import { invoke } from "@tauri-apps/api/core";
import { useAppStore } from "./appStore";

const mockInvoke = invoke as ReturnType<typeof vi.fn>;

beforeEach(() => {
  // Reset store between tests
  useAppStore.setState({
    repos: [],
    activeRepoId: null,
    sidebarCollapsed: false,
    sidebarWidth: 260,
    config: null,
    settingsOpen: false,
  });
  mockInvoke.mockReset();
});

describe("appStore", () => {
  it("hydrate populates repos from backend", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "repo_list")
        return Promise.resolve([
          { id: "r1", name: "repo1", path: "/tmp/repo1", sort_order: 0 },
        ]);
      if (cmd === "config_get")
        return Promise.resolve({
          default_shell: "/bin/bash",
          font_family: "JetBrains Mono",
          font_size: 14,
          scrollback_lines: 10000,
          theme: "dark",
          git_poll_interval_secs: 5,
        });
      if (cmd === "state_load_layout") return Promise.resolve(null);
      if (cmd === "terminal_list") return Promise.resolve([]);
      if (cmd === "terminal_get_active") return Promise.resolve(null);
      if (cmd === "repo_get_all_git_info") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await useAppStore.getState().hydrate();
    expect(useAppStore.getState().repos).toHaveLength(1);
    expect(useAppStore.getState().repos[0].name).toBe("repo1");
  });

  it("addRepo calls invoke and updates state", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "repo_add")
        return Promise.resolve({
          id: "r1",
          name: "myrepo",
          path: "/tmp/myrepo",
          sort_order: 0,
        });
      if (cmd === "terminal_create")
        return Promise.resolve({
          id: "t1",
          repo_id: "r1",
          title: "Terminal 1",
          shell: "/bin/bash",
          sort_order: 0,
        });
      if (cmd === "terminal_set_active") return Promise.resolve();
      return Promise.resolve(null);
    });

    await useAppStore.getState().addRepo("/tmp/myrepo");
    expect(useAppStore.getState().repos).toHaveLength(1);
    expect(useAppStore.getState().activeRepoId).toBe("r1");
  });

  it("removeRepo calls invoke and removes from state", async () => {
    useAppStore.setState({
      repos: [
        {
          id: "r1",
          name: "repo",
          path: "/tmp",
          sortOrder: 0,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [],
          activeTabId: null,
        },
      ],
      activeRepoId: "r1",
    });

    mockInvoke.mockResolvedValue(undefined);
    await useAppStore.getState().removeRepo("r1");
    expect(useAppStore.getState().repos).toHaveLength(0);
    expect(useAppStore.getState().activeRepoId).toBeNull();
  });

  it("setActiveRepo updates activeRepoId", () => {
    useAppStore.getState().setActiveRepo("r2");
    expect(useAppStore.getState().activeRepoId).toBe("r2");
  });

  it("addTab creates tab in correct repo", async () => {
    useAppStore.setState({
      repos: [
        {
          id: "r1",
          name: "repo",
          path: "/tmp",
          sortOrder: 0,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [],
          activeTabId: null,
        },
      ],
      activeRepoId: "r1",
    });

    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "terminal_create")
        return Promise.resolve({
          id: "t1",
          repo_id: "r1",
          title: "Terminal 1",
          shell: "/bin/bash",
          sort_order: 0,
        });
      if (cmd === "terminal_set_active") return Promise.resolve();
      return Promise.resolve(null);
    });

    await useAppStore.getState().addTab("r1");
    expect(useAppStore.getState().repos[0].tabs).toHaveLength(1);
    expect(useAppStore.getState().repos[0].activeTabId).toBe("t1");
  });

  it("closeTab removes tab and selects next", async () => {
    useAppStore.setState({
      repos: [
        {
          id: "r1",
          name: "repo",
          path: "/tmp",
          sortOrder: 0,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [
            {
              id: "t1",
              repoId: "r1",
              name: "Terminal 1",
              shell: null,
              sortOrder: 0,
            },
            {
              id: "t2",
              repoId: "r1",
              name: "Terminal 2",
              shell: null,
              sortOrder: 1,
            },
          ],
          activeTabId: "t1",
        },
      ],
    });

    mockInvoke.mockResolvedValue(undefined);
    await useAppStore.getState().closeTab("r1", "t1");
    expect(useAppStore.getState().repos[0].tabs).toHaveLength(1);
    expect(useAppStore.getState().repos[0].activeTabId).toBe("t2");
  });

  it("closeTab on last tab leaves activeTabId null", async () => {
    useAppStore.setState({
      repos: [
        {
          id: "r1",
          name: "repo",
          path: "/tmp",
          sortOrder: 0,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [
            {
              id: "t1",
              repoId: "r1",
              name: "Terminal 1",
              shell: null,
              sortOrder: 0,
            },
          ],
          activeTabId: "t1",
        },
      ],
    });

    mockInvoke.mockResolvedValue(undefined);
    await useAppStore.getState().closeTab("r1", "t1");
    expect(useAppStore.getState().repos[0].tabs).toHaveLength(0);
    expect(useAppStore.getState().repos[0].activeTabId).toBeNull();
  });

  it("updateGitStatus updates branch and isDirty", () => {
    useAppStore.setState({
      repos: [
        {
          id: "r1",
          name: "repo",
          path: "/tmp",
          sortOrder: 0,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [],
          activeTabId: null,
        },
      ],
    });

    useAppStore.getState().updateGitStatus([
      [
        "r1",
        {
          branch: "main",
          is_dirty: true,
          detached: false,
          ahead: 2,
          behind: 0,
        },
      ],
    ]);

    const repo = useAppStore.getState().repos[0];
    expect(repo.gitInfo.branch).toBe("main");
    expect(repo.gitInfo.is_dirty).toBe(true);
    expect(repo.gitInfo.ahead).toBe(2);
  });

  it("toggleSidebar flips collapsed state", () => {
    expect(useAppStore.getState().sidebarCollapsed).toBe(false);
    useAppStore.getState().toggleSidebar();
    expect(useAppStore.getState().sidebarCollapsed).toBe(true);
    useAppStore.getState().toggleSidebar();
    expect(useAppStore.getState().sidebarCollapsed).toBe(false);
  });
});
