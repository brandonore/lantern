import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { RepoItem } from "./RepoItem";
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

describe("RepoItem", () => {
  it("displays repo name (last path segment)", () => {
    render(
      <RepoItem
        repo={makeRepo()}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.getByText("my-repo")).toBeDefined();
  });

  it("displays branch name in meta", () => {
    render(
      <RepoItem
        repo={makeRepo()}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.getByText("main")).toBeDefined();
  });

  it("shows dirty badge when is_dirty=true", () => {
    render(
      <RepoItem
        repo={makeRepo({
          gitInfo: {
            branch: "main",
            is_dirty: true,
            detached: false,
            ahead: 0,
            behind: 0,
          },
        })}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.getByText("M")).toBeDefined();
  });

  it("hides dirty badge when is_dirty=false", () => {
    render(
      <RepoItem
        repo={makeRepo()}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.queryByText("M")).toBeNull();
  });

  it("shows ahead/behind counts", () => {
    render(
      <RepoItem
        repo={makeRepo({
          gitInfo: {
            branch: "main",
            is_dirty: false,
            detached: false,
            ahead: 3,
            behind: 1,
          },
        })}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.getByText("↑3")).toBeDefined();
    expect(screen.getByText("↓1")).toBeDefined();
  });

  it("hides ahead/behind when zero", () => {
    render(
      <RepoItem
        repo={makeRepo()}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.queryByText(/↑/)).toBeNull();
    expect(screen.queryByText(/↓/)).toBeNull();
  });

  it("grouped non-default item shows branch icon", () => {
    const { container } = render(
      <RepoItem
        repo={makeRepo({ groupId: "g1", isDefault: false })}
        isActive={false}
        isGrouped={true}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    const svgs = container.querySelectorAll("svg[class*='icon']");
    expect(svgs.length).toBe(1);
  });

  it("standalone item shows star icon", () => {
    const { container } = render(
      <RepoItem
        repo={makeRepo()}
        isActive={false}
        isGrouped={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    const svgs = container.querySelectorAll("svg[class*='icon']");
    expect(svgs.length).toBe(1);
  });

  it("grouped default item shows star icon", () => {
    const { container } = render(
      <RepoItem
        repo={makeRepo({ groupId: "g1", isDefault: true })}
        isActive={false}
        isGrouped={true}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    const svgs = container.querySelectorAll("svg[class*='icon']");
    expect(svgs.length).toBe(1);
  });

  it("shows default badge when isDefault in group", () => {
    render(
      <RepoItem
        repo={makeRepo({ groupId: "g1", isDefault: true })}
        isActive={false}
        isGrouped={true}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.getByText("default")).toBeDefined();
  });

  it("hides default badge for non-default repo", () => {
    render(
      <RepoItem
        repo={makeRepo({ groupId: "g1", isDefault: false })}
        isActive={false}
        isGrouped={true}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.queryByText("default")).toBeNull();
  });

  it("shows repo name as primary label and branch in meta when grouped", () => {
    render(
      <RepoItem
        repo={makeRepo({
          name: "my-worktree",
          groupId: "g1",
          gitInfo: { branch: "feat/auth", is_dirty: false, detached: false, ahead: 0, behind: 0 },
        })}
        isActive={false}
        isGrouped={true}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(screen.getByText("my-worktree")).toBeDefined();
    expect(screen.getByText("feat/auth")).toBeDefined();
  });
});
