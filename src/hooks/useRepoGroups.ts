import { useMemo } from "react";
import { useAppStore } from "../stores/appStore";
import type { RepoGroup, RepoWithState } from "../types";

export function useRepoGroups(): RepoGroup[] {
  const repos = useAppStore((s) => s.repos);

  return useMemo(() => {
    const groupMap = new Map<string, RepoWithState[]>();
    const groupOrder: string[] = [];

    for (const repo of repos) {
      const key = repo.groupId ?? `standalone-${repo.id}`;
      if (!groupMap.has(key)) {
        groupMap.set(key, []);
        groupOrder.push(key);
      }
      groupMap.get(key)!.push(repo);
    }

    return groupOrder.map((key) => {
      const members = groupMap.get(key)!;
      const isWorktreeGroup = members[0].groupId !== null;
      const defaultMember = members.find((r) => r.isDefault);

      members.sort((a, b) => {
        if (a.isDefault !== b.isDefault) return a.isDefault ? -1 : 1;
        return a.sortOrder - b.sortOrder;
      });

      return {
        groupId: key,
        name: defaultMember?.name ?? members[0].name,
        repos: members,
        isWorktreeGroup,
      };
    });
  }, [repos]);
}
