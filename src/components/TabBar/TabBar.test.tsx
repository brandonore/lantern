import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useAppStore } from "../../stores/appStore";
import { TabBar } from "./TabBar";
import type { RepoWithState } from "../../types";

const makeRepo = (overrides: Partial<RepoWithState> = {}): RepoWithState => ({
  id: "r1",
  name: "my-repo",
  path: "/tmp",
  sortOrder: 0,
  groupId: null,
  isDefault: false,
  gitInfo: {
    branch: "main",
    is_dirty: false,
    detached: false,
    ahead: 0,
    behind: 0,
  },
  tabs: [
    { id: "t1", repoId: "r1", name: "Terminal 1", shell: null, sortOrder: 0 },
    { id: "t2", repoId: "r1", name: "Terminal 2", shell: null, sortOrder: 1 },
  ],
  activeTabId: "t1",
  ...overrides,
});

beforeEach(() => {
  useAppStore.setState({
    repos: [makeRepo()],
    activeRepoId: "r1",
    config: null,
    settingsOpen: false,
    sidebarCollapsed: false,
    sidebarWidth: 260,
  });
});

describe("TabBar", () => {
  it("renders tabs for active repo", () => {
    render(<TabBar />);
    expect(screen.getByText("Terminal 1")).toBeDefined();
    expect(screen.getByText("Terminal 2")).toBeDefined();
  });

  it("highlights active tab", () => {
    render(<TabBar />);
    const tab1 = screen.getByText("Terminal 1").closest("[class*='tab']");
    expect(tab1?.className).toContain("active");
  });

  it('calls addTab on "+" click', () => {
    const addTab = vi.fn().mockResolvedValue({
      id: "t3",
      repoId: "r1",
      name: "Terminal 3",
      shell: null,
      sortOrder: 2,
    });
    useAppStore.setState({ addTab });
    render(<TabBar />);
    fireEvent.click(screen.getByTitle("New terminal (Ctrl+T)"));
    expect(addTab).toHaveBeenCalledWith("r1");
  });

  it("calls closeTab on X click", () => {
    const closeTab = vi.fn().mockResolvedValue(undefined);
    useAppStore.setState({ closeTab });
    render(<TabBar />);
    const closeButtons = screen.getAllByTitle("Close terminal (Ctrl+W)");
    fireEvent.click(closeButtons[0]);
    expect(closeTab).toHaveBeenCalledWith("r1", "t1");
  });

  it("shows no tabs when repo has none", () => {
    useAppStore.setState({
      repos: [makeRepo({ tabs: [], activeTabId: null })],
    });
    render(<TabBar />);
    expect(screen.queryByText("Terminal 1")).toBeNull();
  });
});
