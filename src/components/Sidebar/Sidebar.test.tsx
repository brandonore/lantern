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
  groupId: null,
  isDefault: false,
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
    collapsedGroupIds: [],
  });
});

describe("Sidebar", () => {
  it("renders repo list from store", () => {
    useAppStore.setState({
      repos: [makeRepo(), makeRepo({ id: "r2", name: "other-repo" })],
    });
    render(<Sidebar />);
    // Header + item for each
    expect(screen.getAllByText("my-repo").length).toBe(2);
    expect(screen.getAllByText("other-repo").length).toBe(2);
  });

  it("highlights active repo", () => {
    useAppStore.setState({
      repos: [makeRepo()],
      activeRepoId: "r1",
    });
    render(<Sidebar />);
    const repoItem = screen.getAllByText("my-repo")[1].closest("[class*='repoItem']");
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
    // Click the repo item (second instance, first is the header)
    fireEvent.click(screen.getAllByText("my-repo")[1]);
    expect(setActiveRepo).toHaveBeenCalledWith("r1");
  });

  it('shows "Add repository" button', () => {
    render(<Sidebar />);
    expect(screen.getByText("+ Add repository")).toBeDefined();
  });

  it("renders grouped repos with header", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "project", groupId: "g1", isDefault: true }),
        makeRepo({ id: "r2", name: "feat-wt", groupId: "g1", gitInfo: { branch: "feat", is_dirty: false, detached: false, ahead: 0, behind: 0 } }),
        makeRepo({ id: "r3", name: "fix-wt", groupId: "g1", gitInfo: { branch: "fix", is_dirty: false, detached: false, ahead: 0, behind: 0 } }),
      ],
    });
    render(<Sidebar />);
    // Group header + default repo item both show "project"
    expect(screen.getAllByText("project").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("feat-wt")).toBeDefined();
    expect(screen.getByText("fix-wt")).toBeDefined();
  });

  it("renders standalone repos with group headers", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "repo-a" }),
        makeRepo({ id: "r2", name: "repo-b" }),
      ],
    });
    render(<Sidebar />);
    expect(screen.getAllByText("repo-a").length).toBe(2);
    expect(screen.getAllByText("repo-b").length).toBe(2);
  });

  it("renders dividers between groups", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "repo-a" }),
        makeRepo({ id: "r2", name: "repo-b" }),
      ],
    });
    const { container } = render(<Sidebar />);
    // Each group div + each groupHeader div
    const groups = container.querySelectorAll("[class*='group']");
    expect(groups.length).toBeGreaterThanOrEqual(2);
  });

  it("renders mixed groups and standalone", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "project", groupId: "g1", isDefault: true }),
        makeRepo({ id: "r2", name: "feat-wt", groupId: "g1" }),
        makeRepo({ id: "r3", name: "standalone" }),
      ],
    });
    render(<Sidebar />);
    expect(screen.getAllByText("project").length).toBeGreaterThanOrEqual(2);
    expect(screen.getAllByText("standalone").length).toBe(2);
  });

  it("shows default badge for default repo in group", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "project", groupId: "g1", isDefault: true }),
        makeRepo({ id: "r2", name: "feat-wt", groupId: "g1" }),
      ],
    });
    render(<Sidebar />);
    expect(screen.getByText("default")).toBeDefined();
  });
});
