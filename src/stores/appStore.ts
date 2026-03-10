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

interface AppStore {
  // State
  repos: RepoWithState[];
  activeRepoId: string | null;
  sidebarCollapsed: boolean;
  sidebarWidth: number;
  config: UserConfig | null;
  settingsOpen: boolean;

  // Actions
  hydrate: () => Promise<void>;
  addRepo: (path: string) => Promise<void>;
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
}

export const useAppStore = create<AppStore>()(
  immer((set, get) => ({
    repos: [],
    activeRepoId: null,
    sidebarCollapsed: false,
    sidebarWidth: 260,
    config: null,
    settingsOpen: false,

    hydrate: async () => {
      const [repos, config, layout] = await Promise.all([
        cmd.repoList(),
        cmd.configGet(),
        cmd.stateLoadLayout(),
      ]);

      // Load tabs and active tab for each repo
      const reposWithState: RepoWithState[] = await Promise.all(
        repos.map(async (repo: Repo) => {
          const tabs = await cmd.terminalList(repo.id);
          const activeTabId = await cmd.terminalGetActive(repo.id);
          return {
            ...repo,
            gitInfo: {
              branch: null,
              is_dirty: false,
              detached: false,
              ahead: 0,
              behind: 0,
            },
            tabs: tabs.map((t: any) => ({
              id: t.id,
              repoId: t.repo_id,
              name: t.title,
              shell: t.shell,
              sortOrder: t.sort_order,
            })),
            activeTabId,
          };
        })
      );

      set((state) => {
        state.repos = reposWithState;
        state.config = config;
        if (layout) {
          state.sidebarWidth = layout.sidebar_width;
          state.activeRepoId = layout.active_repo_id;
        }
        // Default to first repo if no active repo
        if (!state.activeRepoId && reposWithState.length > 0) {
          state.activeRepoId = reposWithState[0].id;
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
      const repo = await cmd.repoAdd(path);
      const tab = await cmd.terminalCreate(repo.id);

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
          tabs: [
            {
              id: tab.id,
              repoId: tab.repo_id,
              name: tab.title,
              shell: tab.shell,
              sortOrder: tab.sort_order,
            },
          ],
          activeTabId: tab.id,
        };
        state.repos.push(newRepo);
        state.activeRepoId = repo.id;
      });

      await cmd.terminalSetActive(repo.id, tab.id);
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
      const session = await cmd.terminalCreate(repoId);
      const tab: TerminalTab = {
        id: session.id,
        repoId: session.repo_id,
        name: session.title,
        shell: session.shell,
        sortOrder: session.sort_order,
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
  }))
);
