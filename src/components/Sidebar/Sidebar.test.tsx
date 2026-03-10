import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useAppStore } from "../../stores/appStore";
import { Sidebar } from "./Sidebar";
import type { RepoWithState } from "../../types";

const makeRepo = (overrides: Partial<RepoWithState> = {}): RepoWithState => ({
  id: "r1",
  name: "my-repo",
  path: "/home/user/my-repo",
  sortOrder: 0,
  gitInfo: {
    branch: "main",
    is_dirty: false,
    detached: false,
    ahead: 0,
    behind: 0,
  },
  tabs: [],
  activeTabId: null,
  ...overrides,
});

beforeEach(() => {
  useAppStore.setState({
    repos: [],
    activeRepoId: null,
    sidebarCollapsed: false,
    sidebarWidth: 260,
    config: null,
    settingsOpen: false,
  });
});

describe("Sidebar", () => {
  it("renders repo list from store", () => {
    useAppStore.setState({
      repos: [makeRepo(), makeRepo({ id: "r2", name: "other-repo" })],
    });
    render(<Sidebar />);
    expect(screen.getByText("my-repo")).toBeDefined();
    expect(screen.getByText("other-repo")).toBeDefined();
  });

  it("highlights active repo", () => {
    useAppStore.setState({
      repos: [makeRepo()],
      activeRepoId: "r1",
    });
    render(<Sidebar />);
    const repoItem = screen.getByText("my-repo").closest("[class*='repoItem']");
    expect(repoItem?.className).toContain("active");
  });

  it("calls setActiveRepo on click", () => {
    const setActiveRepo = vi.fn();
    useAppStore.setState({
      repos: [makeRepo()],
      activeRepoId: null,
      setActiveRepo,
    });
    render(<Sidebar />);
    fireEvent.click(screen.getByText("my-repo"));
    expect(setActiveRepo).toHaveBeenCalledWith("r1");
  });

  it('shows "Add repository" button', () => {
    render(<Sidebar />);
    expect(screen.getByText("+ Add repository")).toBeDefined();
  });
});
