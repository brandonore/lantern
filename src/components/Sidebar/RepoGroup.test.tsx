import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useAppStore } from "../../stores/appStore";
import { RepoGroup } from "./RepoGroup";
import type { RepoGroup as RepoGroupType, RepoWithState } from "../../types";

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
    collapsedGroupIds: [],
  });
});

describe("RepoGroup", () => {
  it("worktree group renders header and children", () => {
    const group: RepoGroupType = {
      groupId: "g1",
      name: "lantern",
      isWorktreeGroup: true,
      repos: [
        makeRepo({ id: "r1", name: "lantern", groupId: "g1", isDefault: true }),
        makeRepo({ id: "r2", name: "feat", groupId: "g1", gitInfo: { branch: "feat/auth", is_dirty: false, detached: false, ahead: 0, behind: 0 } }),
        makeRepo({ id: "r3", name: "fix", groupId: "g1", gitInfo: { branch: "fix/pty", is_dirty: false, detached: false, ahead: 0, behind: 0 } }),
      ],
    };
    render(<RepoGroup group={group} />);
    // Header + repo item both say "lantern"
    expect(screen.getAllByText("lantern").length).toBeGreaterThanOrEqual(1);
    // Repo folder names shown, branch names in meta row
    expect(screen.getByText("feat")).toBeDefined();
    expect(screen.getByText("feat/auth")).toBeDefined();
    expect(screen.getByText("fix")).toBeDefined();
    expect(screen.getByText("fix/pty")).toBeDefined();
  });

  it("standalone group renders header and single item", () => {
    const group: RepoGroupType = {
      groupId: "standalone-r1",
      name: "solo-repo",
      isWorktreeGroup: false,
      repos: [makeRepo({ id: "r1", name: "solo-repo" })],
    };
    render(<RepoGroup group={group} />);
    // Header + repo item both show the name
    expect(screen.getAllByText("solo-repo").length).toBe(2);
  });

  it("group header shows group name", () => {
    const group: RepoGroupType = {
      groupId: "g1",
      name: "my-project",
      isWorktreeGroup: true,
      repos: [
        makeRepo({ id: "r1", name: "my-project", groupId: "g1", isDefault: true }),
      ],
    };
    render(<RepoGroup group={group} />);
    // Header and the branch-as-label item
    const elements = screen.getAllByText("my-project");
    // At least the header text should be present
    expect(elements.length).toBeGreaterThanOrEqual(1);
  });

  it("click on child calls setActiveRepo", () => {
    const setActiveRepo = vi.fn();
    useAppStore.setState({ setActiveRepo });
    const group: RepoGroupType = {
      groupId: "g1",
      name: "project",
      isWorktreeGroup: true,
      repos: [
        makeRepo({ id: "r1", name: "main-repo", groupId: "g1", isDefault: true }),
      ],
    };
    render(<RepoGroup group={group} />);
    fireEvent.click(screen.getByText("main-repo"));
    expect(setActiveRepo).toHaveBeenCalledWith("r1");
  });

  it("remove on child calls removeRepo", () => {
    const removeRepo = vi.fn().mockResolvedValue(undefined);
    useAppStore.setState({ removeRepo });
    const group: RepoGroupType = {
      groupId: "standalone-r1",
      name: "repo",
      isWorktreeGroup: false,
      repos: [makeRepo({ id: "r1", name: "repo" })],
    };
    render(<RepoGroup group={group} />);
    fireEvent.click(screen.getByTitle("Remove repository"));
    expect(removeRepo).toHaveBeenCalledWith("r1");
  });

  it("clicking header collapses children", () => {
    const toggleGroupCollapsed = vi.fn();
    useAppStore.setState({ toggleGroupCollapsed });
    const group: RepoGroupType = {
      groupId: "g1",
      name: "project",
      isWorktreeGroup: true,
      repos: [
        makeRepo({ id: "r1", name: "main-repo", groupId: "g1", isDefault: true }),
      ],
    };
    render(<RepoGroup group={group} />);
    // Children visible initially
    expect(screen.getByText("main-repo")).toBeDefined();
    // Click header to toggle
    fireEvent.click(screen.getByText("project"));
    expect(toggleGroupCollapsed).toHaveBeenCalledWith("g1");
  });

  it("hides children when collapsed", () => {
    useAppStore.setState({
      collapsedGroupIds: ["g1"],
    });
    const group: RepoGroupType = {
      groupId: "g1",
      name: "project",
      isWorktreeGroup: true,
      repos: [
        makeRepo({ id: "r1", name: "main-repo", groupId: "g1", isDefault: true }),
      ],
    };
    render(<RepoGroup group={group} />);
    // Header should still be visible
    expect(screen.getByText("project")).toBeDefined();
    // Children should be hidden
    expect(screen.queryByText("main-repo")).toBeNull();
  });
});
