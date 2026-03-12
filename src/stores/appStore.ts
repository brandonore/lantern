import { create } from "zustand";
import { immer } from "zustand/middleware/immer";
import type {
  Repo,
  TerminalTab,
  GitInfo,
  RepoWithState,
  UserConfig,
} from "../types";
import * as cmd from "../lib/tauriCommands";

function normalizeActiveTabId(
  tabs: TerminalTab[],
  activeTabId: string | null
): string | null {
  if (tabs.length === 0) return null;
  if (activeTabId && tabs.some((tab) => tab.id === activeTabId)) {
    return activeTabId;
  }
  return tabs[0].id;
}

function normalizeActiveRepoId(
  repos: RepoWithState[],
  activeRepoId: string | null
): string | null {
  if (repos.length === 0) return null;
  if (activeRepoId && repos.some((repo) => repo.id === activeRepoId)) {
    return activeRepoId;
  }
  return repos[0].id;
}

interface AppStore {
  // State
  repos: RepoWithState[];
  activeRepoId: string | null;
  sidebarCollapsed: boolean;
  sidebarWidth: number;
  config: UserConfig | null;
  settingsOpen: boolean;
  renamingTabId: string | null;
  searchOpen: boolean;
  searchQuery: string;
  collapsedGroupIds: string[];

  // Actions
  hydrate: () => Promise<void>;
  addRepo: (path: string) => Promise<void>;
  addRepoWithWorktrees: (path: string) => Promise<void>;
  removeRepo: (id: string) => Promise<void>;
  setActiveRepo: (id: string) => void;
  reorderRepos: (ids: string[]) => Promise<void>;

  addTab: (repoId: string) => Promise<TerminalTab>;
  closeTab: (repoId: string, tabId: string) => Promise<void>;
  setActiveTab: (repoId: string, tabId: string) => void;
  renameTab: (tabId: string, name: string) => Promise<void>;

  updateGitStatus: (updates: [string, GitInfo][]) => void;
  toggleSidebar: () => void;
  setSidebarWidth: (width: number) => void;
  setConfig: (config: UserConfig) => void;
  setSettingsOpen: (open: boolean) => void;
  setRenamingTabId: (id: string | null) => void;
  setSearchOpen: (open: boolean) => void;
  setSearchQuery: (query: string) => void;
  toggleGroupCollapsed: (groupId: string) => void;
}

export const useAppStore = create<AppStore>()(
  immer((set, get) => ({
    repos: [],
    activeRepoId: null,
    sidebarCollapsed: false,
    sidebarWidth: 260,
    config: null,
    settingsOpen: false,
    renamingTabId: null,
    searchOpen: false,
    searchQuery: "",
    collapsedGroupIds: [] as string[],

    hydrate: async () => {
      const [repos, config, layout] = await Promise.all([
        cmd.repoList(),
        cmd.configGet(),
        cmd.stateLoadLayout(),
      ]);
      const activeTabRepairs: Array<Promise<void>> = [];

      // Load tabs and active tab for each repo
      const reposWithState: RepoWithState[] = await Promise.all(
        repos.map(async (repo: any) => {
          const tabs = await cmd.terminalList(repo.id);
          const mappedTabs = tabs.map((t: any) => ({
            id: t.id,
            repoId: t.repo_id,
            name: t.title,
            shell: t.shell,
            sortOrder: t.sort_order,
          }));
          const persistedActiveTabId = await cmd.terminalGetActive(repo.id);
          const activeTabId = normalizeActiveTabId(mappedTabs, persistedActiveTabId);
          if (activeTabId && activeTabId !== persistedActiveTabId) {
            activeTabRepairs.push(
              cmd.terminalSetActive(repo.id, activeTabId).catch(() => {})
            );
          }
          return {
            id: repo.id,
            name: repo.name,
            path: repo.path,
            sortOrder: repo.sort_order ?? repo.sortOrder ?? 0,
            groupId: repo.group_id ?? repo.groupId ?? null,
            isDefault: repo.is_default ?? repo.isDefault ?? false,
            gitInfo: {
              branch: null,
              is_dirty: false,
              detached: false,
              ahead: 0,
              behind: 0,
            },
            tabs: mappedTabs,
            activeTabId,
          };
        })
      );
      await Promise.all(activeTabRepairs);
      const normalizedActiveRepoId = normalizeActiveRepoId(
        reposWithState,
        layout?.active_repo_id ?? null
      );

      set((state) => {
        state.repos = reposWithState;
        state.config = config;
        if (layout) {
          state.sidebarWidth = layout.sidebar_width;
          state.sidebarCollapsed = layout.sidebar_collapsed;
          state.activeRepoId = normalizedActiveRepoId;
          if (layout.collapsed_group_ids) {
            state.collapsedGroupIds = layout.collapsed_group_ids;
          }
        } else {
          state.activeRepoId = normalizedActiveRepoId;
        }
      });

      // Initial git refresh
      try {
        const gitInfos = await cmd.repoGetAllGitInfo();
        get().updateGitStatus(gitInfos);
      } catch {
        // Ignore initial git errors
      }
    },

    addRepo: async (path) => {
      const raw: any = await cmd.repoAdd(path);
      const repo: Repo = {
        id: raw.id,
        name: raw.name,
        path: raw.path,
        sortOrder: raw.sort_order ?? 0,
        groupId: raw.group_id ?? null,
        isDefault: raw.is_default ?? false,
      };
      const rawTab: any = await cmd.terminalCreate(repo.id);
      const tab: TerminalTab = {
        id: rawTab.id,
        repoId: rawTab.repo_id,
        name: rawTab.title,
        shell: rawTab.shell,
        sortOrder: rawTab.sort_order,
      };

      set((state) => {
        const newRepo: RepoWithState = {
          ...repo,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [tab],
          activeTabId: tab.id,
        };
        state.repos.push(newRepo);
        state.activeRepoId = repo.id;
      });

      await cmd.terminalSetActive(repo.id, tab.id);
    },

    addRepoWithWorktrees: async (path) => {
      const rawRepos: any[] = await cmd.repoAddWithWorktrees(path);
      const newRepos: RepoWithState[] = [];

      for (const raw of rawRepos) {
        const rawTab: any = await cmd.terminalCreate(raw.id);
        const tab: TerminalTab = {
          id: rawTab.id,
          repoId: rawTab.repo_id,
          name: rawTab.title,
          shell: rawTab.shell,
          sortOrder: rawTab.sort_order,
        };
        await cmd.terminalSetActive(raw.id, tab.id);
        newRepos.push({
          id: raw.id,
          name: raw.name,
          path: raw.path,
          sortOrder: raw.sort_order ?? 0,
          groupId: raw.group_id ?? null,
          isDefault: raw.is_default ?? false,
          gitInfo: {
            branch: null,
            is_dirty: false,
            detached: false,
            ahead: 0,
            behind: 0,
          },
          tabs: [tab],
          activeTabId: tab.id,
        });
      }

      set((state) => {
        state.repos.push(...newRepos);
        if (newRepos.length > 0) {
          state.activeRepoId = newRepos[0].id;
        }
      });
    },

    removeRepo: async (id) => {
      await cmd.repoRemove(id);
      set((state) => {
        state.repos = state.repos.filter((r) => r.id !== id);
        if (state.activeRepoId === id) {
          state.activeRepoId = state.repos[0]?.id ?? null;
        }
      });
    },

    setActiveRepo: (id) => {
      set((state) => {
        state.activeRepoId = id;
      });
    },

    reorderRepos: async (ids) => {
      await cmd.repoReorder(ids);
      set((state) => {
        const repoMap = new Map(state.repos.map((r) => [r.id, r]));
        state.repos = ids.map((id) => repoMap.get(id)!).filter(Boolean);
      });
    },

    addTab: async (repoId) => {
      const raw: any = await cmd.terminalCreate(repoId);
      const tab: TerminalTab = {
        id: raw.id,
        repoId: raw.repo_id,
        name: raw.title,
        shell: raw.shell,
        sortOrder: raw.sort_order,
      };

      set((state) => {
        const repo = state.repos.find((r) => r.id === repoId);
        if (repo) {
          repo.tabs.push(tab);
          repo.activeTabId = tab.id;
        }
      });

      await cmd.terminalSetActive(repoId, tab.id);
      return tab;
    },

    closeTab: async (repoId, tabId) => {
      await cmd.terminalClose(tabId);

      set((state) => {
        const repo = state.repos.find((r) => r.id === repoId);
        if (!repo) return;

        const idx = repo.tabs.findIndex((t) => t.id === tabId);
        repo.tabs = repo.tabs.filter((t) => t.id !== tabId);

        if (repo.activeTabId === tabId) {
          if (repo.tabs.length > 0) {
            // Select next tab, or previous if at end
            const newIdx = Math.min(idx, repo.tabs.length - 1);
            repo.activeTabId = repo.tabs[newIdx].id;
          } else {
            repo.activeTabId = null;
          }
        }
      });
    },

    setActiveTab: (repoId, tabId) => {
      set((state) => {
        const repo = state.repos.find((r) => r.id === repoId);
        if (repo) {
          repo.activeTabId = tabId;
        }
      });
      cmd.terminalSetActive(repoId, tabId).catch(console.error);
    },

    renameTab: async (tabId, name) => {
      await cmd.terminalRename(tabId, name);
      set((state) => {
        for (const repo of state.repos) {
          const tab = repo.tabs.find((t) => t.id === tabId);
          if (tab) {
            tab.name = name;
            break;
          }
        }
      });
    },

    updateGitStatus: (updates) => {
      set((state) => {
        for (const [repoId, info] of updates) {
          const repo = state.repos.find((r) => r.id === repoId);
          if (repo) {
            repo.gitInfo = info;
          }
        }
      });
    },

    toggleSidebar: () => {
      set((state) => {
        state.sidebarCollapsed = !state.sidebarCollapsed;
      });
    },

    setSidebarWidth: (width) => {
      set((state) => {
        state.sidebarWidth = width;
      });
    },

    setConfig: (config) => {
      set((state) => {
        state.config = config;
      });
    },

    setSettingsOpen: (open) => {
      set((state) => {
        state.settingsOpen = open;
      });
    },

    setRenamingTabId: (id) => {
      set((state) => {
        state.renamingTabId = id;
      });
    },

    setSearchOpen: (open) => {
      set((state) => {
        state.searchOpen = open;
        if (!open) {
          state.searchQuery = "";
        }
      });
    },

    setSearchQuery: (query) => {
      set((state) => {
        state.searchQuery = query;
      });
    },

    toggleGroupCollapsed: (groupId) => {
      set((state) => {
        const idx = state.collapsedGroupIds.indexOf(groupId);
        if (idx >= 0) {
          state.collapsedGroupIds.splice(idx, 1);
        } else {
          state.collapsedGroupIds.push(groupId);
        }
      });
    },
  }))
);
