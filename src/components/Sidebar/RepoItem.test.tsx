import { describe, it, expect, vi } from "vitest";
import { render, screen } from "@testing-library/react";
import { RepoItem } from "./RepoItem";
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

  it("displays branch name", () => {
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

  it("shows dirty dot when isDirty=true", () => {
    const { container } = render(
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
    expect(container.querySelector("[class*='dirtyDot']")).not.toBeNull();
  });

  it("hides dirty dot when isDirty=false", () => {
    const { container } = render(
      <RepoItem
        repo={makeRepo()}
        isActive={false}
        onClick={vi.fn()}
        onRemove={vi.fn()}
      />
    );
    expect(container.querySelector("[class*='dirtyDot']")).toBeNull();
  });
});
