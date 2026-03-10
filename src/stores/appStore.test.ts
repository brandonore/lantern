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
          ui_scale: 1,
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

  it("hydrate restores the saved sidebar collapsed state", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "repo_list") return Promise.resolve([]);
      if (cmd === "config_get")
        return Promise.resolve({
          default_shell: "/bin/bash",
          font_family: "JetBrains Mono",
          font_size: 14,
          scrollback_lines: 10000,
          theme: "dark",
          git_poll_interval_secs: 5,
          ui_scale: 1,
        });
      if (cmd === "state_load_layout")
        return Promise.resolve({
          window_x: null,
          window_y: null,
          window_width: 1200,
          window_height: 800,
          window_maximized: false,
          sidebar_width: 300,
          sidebar_collapsed: true,
          active_repo_id: null,
        });
      if (cmd === "repo_get_all_git_info") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await useAppStore.getState().hydrate();

    expect(useAppStore.getState().sidebarWidth).toBe(300);
    expect(useAppStore.getState().sidebarCollapsed).toBe(true);
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
          groupId: null,
          isDefault: false,
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
          groupId: null,
          isDefault: false,
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
          groupId: null,
          isDefault: false,
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
          groupId: null,
          isDefault: false,
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
          groupId: null,
          isDefault: false,
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

  it("hydrate maps group fields from backend", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "repo_list")
        return Promise.resolve([
          {
            id: "r1",
            name: "project",
            path: "/tmp/project",
            sort_order: 0,
            group_id: "g1",
            is_default: true,
          },
          {
            id: "r2",
            name: "feat-wt",
            path: "/tmp/feat-wt",
            sort_order: 1,
            group_id: "g1",
            is_default: false,
          },
        ]);
      if (cmd === "config_get")
        return Promise.resolve({
          default_shell: "/bin/bash",
          font_family: "JetBrains Mono",
          font_size: 14,
          scrollback_lines: 10000,
          theme: "dark",
          git_poll_interval_secs: 5,
          ui_scale: 1,
        });
      if (cmd === "state_load_layout") return Promise.resolve(null);
      if (cmd === "terminal_list") return Promise.resolve([]);
      if (cmd === "terminal_get_active") return Promise.resolve(null);
      if (cmd === "repo_get_all_git_info") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await useAppStore.getState().hydrate();
    const repos = useAppStore.getState().repos;
    expect(repos).toHaveLength(2);
    expect(repos[0].groupId).toBe("g1");
    expect(repos[0].isDefault).toBe(true);
    expect(repos[1].groupId).toBe("g1");
    expect(repos[1].isDefault).toBe(false);
  });

  it("hydrate handles missing group fields (backward compat)", async () => {
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
          ui_scale: 1,
        });
      if (cmd === "state_load_layout") return Promise.resolve(null);
      if (cmd === "terminal_list") return Promise.resolve([]);
      if (cmd === "terminal_get_active") return Promise.resolve(null);
      if (cmd === "repo_get_all_git_info") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await useAppStore.getState().hydrate();
    const repo = useAppStore.getState().repos[0];
    expect(repo.groupId).toBeNull();
    expect(repo.isDefault).toBe(false);
  });

  it("addRepoWithWorktrees adds multiple repos", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "repo_add_with_worktrees")
        return Promise.resolve([
          { id: "r1", name: "main", path: "/tmp/main", sort_order: 0, group_id: "g1", is_default: true },
          { id: "r2", name: "feat", path: "/tmp/feat", sort_order: 1, group_id: "g1", is_default: false },
          { id: "r3", name: "fix", path: "/tmp/fix", sort_order: 2, group_id: "g1", is_default: false },
        ]);
      if (cmd === "terminal_create") {
        return Promise.resolve({
          id: `t-${Math.random()}`,
          repo_id: "r1",
          title: "Terminal 1",
          shell: "/bin/bash",
          sort_order: 0,
        });
      }
      if (cmd === "terminal_set_active") return Promise.resolve();
      return Promise.resolve(null);
    });

    await useAppStore.getState().addRepoWithWorktrees("/tmp/main");
    const repos = useAppStore.getState().repos;
    expect(repos).toHaveLength(3);
    expect(repos[0].groupId).toBe("g1");
    expect(repos[0].isDefault).toBe(true);
    expect(repos[1].groupId).toBe("g1");
    expect(repos[2].groupId).toBe("g1");
  });

  it("toggleGroupCollapsed toggles group in array", () => {
    expect(useAppStore.getState().collapsedGroupIds).toHaveLength(0);
    useAppStore.getState().toggleGroupCollapsed("g1");
    expect(useAppStore.getState().collapsedGroupIds).toContain("g1");
    useAppStore.getState().toggleGroupCollapsed("g1");
    expect(useAppStore.getState().collapsedGroupIds).not.toContain("g1");
  });

  it("hydrate restores collapsed group ids from layout", async () => {
    mockInvoke.mockImplementation((cmd: string) => {
      if (cmd === "repo_list") return Promise.resolve([]);
      if (cmd === "config_get")
        return Promise.resolve({
          default_shell: "/bin/bash",
          font_family: "JetBrains Mono",
          font_size: 14,
          scrollback_lines: 10000,
          theme: "dark",
          git_poll_interval_secs: 5,
          ui_scale: 1,
        });
      if (cmd === "state_load_layout")
        return Promise.resolve({
          window_x: null,
          window_y: null,
          window_width: 1200,
          window_height: 800,
          window_maximized: false,
          sidebar_width: 250,
          sidebar_collapsed: false,
          active_repo_id: null,
          collapsed_group_ids: ["g1", "standalone-r2"],
        });
      if (cmd === "repo_get_all_git_info") return Promise.resolve([]);
      return Promise.resolve(null);
    });

    await useAppStore.getState().hydrate();
    const collapsed = useAppStore.getState().collapsedGroupIds;
    expect(collapsed).toContain("g1");
    expect(collapsed).toContain("standalone-r2");
    expect(collapsed).toHaveLength(2);
  });

  it("removeRepo from group keeps others", async () => {
    useAppStore.setState({
      repos: [
        {
          id: "r1",
          name: "main",
          path: "/tmp/main",
          sortOrder: 0,
          groupId: "g1",
          isDefault: true,
          gitInfo: { branch: null, is_dirty: false, detached: false, ahead: 0, behind: 0 },
          tabs: [],
          activeTabId: null,
        },
        {
          id: "r2",
          name: "feat",
          path: "/tmp/feat",
          sortOrder: 1,
          groupId: "g1",
          isDefault: false,
          gitInfo: { branch: null, is_dirty: false, detached: false, ahead: 0, behind: 0 },
          tabs: [],
          activeTabId: null,
        },
        {
          id: "r3",
          name: "fix",
          path: "/tmp/fix",
          sortOrder: 2,
          groupId: "g1",
          isDefault: false,
          gitInfo: { branch: null, is_dirty: false, detached: false, ahead: 0, behind: 0 },
          tabs: [],
          activeTabId: null,
        },
      ],
      activeRepoId: "r1",
    });

    mockInvoke.mockResolvedValue(undefined);
    await useAppStore.getState().removeRepo("r1");
    const repos = useAppStore.getState().repos;
    expect(repos).toHaveLength(2);
    expect(repos[0].groupId).toBe("g1");
    expect(repos[1].groupId).toBe("g1");
  });
});
