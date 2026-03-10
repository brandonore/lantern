import { describe, it, expect, beforeEach } from "vitest";
import { renderHook } from "@testing-library/react";
import { useAppStore } from "../stores/appStore";
import { useRepoGroups } from "./useRepoGroups";
import type { RepoWithState } from "../types";

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
  });
});

describe("useRepoGroups", () => {
  it("returns empty groups for empty repos", () => {
    const { result } = renderHook(() => useRepoGroups());
    expect(result.current).toHaveLength(0);
  });

  it("standalone repos each get own group", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "repo-a" }),
        makeRepo({ id: "r2", name: "repo-b" }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    expect(result.current).toHaveLength(2);
    expect(result.current[0].isWorktreeGroup).toBe(false);
    expect(result.current[1].isWorktreeGroup).toBe(false);
  });

  it("grouped repos form single group", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "main", groupId: "g1", isDefault: true }),
        makeRepo({ id: "r2", name: "feat", groupId: "g1" }),
        makeRepo({ id: "r3", name: "fix", groupId: "g1" }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    expect(result.current).toHaveLength(1);
    expect(result.current[0].isWorktreeGroup).toBe(true);
    expect(result.current[0].repos).toHaveLength(3);
  });

  it("handles mixed standalone and grouped", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "standalone-a" }),
        makeRepo({ id: "r2", name: "main", groupId: "g1", isDefault: true }),
        makeRepo({ id: "r3", name: "feat", groupId: "g1" }),
        makeRepo({ id: "r4", name: "standalone-b" }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    // standalone-a, g1 group, standalone-b
    expect(result.current).toHaveLength(3);
    expect(result.current[0].isWorktreeGroup).toBe(false);
    expect(result.current[1].isWorktreeGroup).toBe(true);
    expect(result.current[1].repos).toHaveLength(2);
    expect(result.current[2].isWorktreeGroup).toBe(false);
  });

  it("default member sorted first", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "feat", groupId: "g1", sortOrder: 0 }),
        makeRepo({ id: "r2", name: "main", groupId: "g1", isDefault: true, sortOrder: 1 }),
        makeRepo({ id: "r3", name: "fix", groupId: "g1", sortOrder: 2 }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    expect(result.current[0].repos[0].id).toBe("r2");
    expect(result.current[0].repos[0].isDefault).toBe(true);
  });

  it("group name from default member", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "feat", groupId: "g1" }),
        makeRepo({ id: "r2", name: "lantern", groupId: "g1", isDefault: true }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    expect(result.current[0].name).toBe("lantern");
  });

  it("group name falls back to first member", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "alpha", groupId: "g1" }),
        makeRepo({ id: "r2", name: "beta", groupId: "g1" }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    expect(result.current[0].name).toBe("alpha");
  });

  it("preserves sort order within group (after default)", () => {
    useAppStore.setState({
      repos: [
        makeRepo({ id: "r1", name: "c", groupId: "g1", sortOrder: 2 }),
        makeRepo({ id: "r2", name: "a", groupId: "g1", sortOrder: 0 }),
        makeRepo({ id: "r3", name: "b", groupId: "g1", sortOrder: 1 }),
      ],
    });
    const { result } = renderHook(() => useRepoGroups());
    const ids = result.current[0].repos.map((r) => r.id);
    expect(ids).toEqual(["r2", "r3", "r1"]);
  });
});
