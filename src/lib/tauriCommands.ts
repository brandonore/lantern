import { invoke, Channel } from "@tauri-apps/api/core";
import type {
  Repo,
  TerminalTab,
  GitInfo,
  AppLayout,
  UserConfig,
  ProcessInfo,
  TerminalOutputData,
} from "../types";

// ── Repo ──

export async function repoAdd(path: string): Promise<Repo> {
  return invoke("repo_add", { path });
}

export async function repoRemove(id: string): Promise<void> {
  return invoke("repo_remove", { id });
}

export async function repoList(): Promise<Repo[]> {
  return invoke("repo_list");
}

export async function repoReorder(ids: string[]): Promise<void> {
  return invoke("repo_reorder", { ids });
}

export async function repoGetAllGitInfo(): Promise<[string, GitInfo][]> {
  return invoke("repo_get_all_git_info");
}

// ── Terminal ──

export async function terminalCreate(repoId: string): Promise<TerminalTab> {
  return invoke("terminal_create", { repoId });
}

export async function terminalList(repoId: string): Promise<TerminalTab[]> {
  return invoke("terminal_list", { repoId });
}

export async function terminalClose(sessionId: string): Promise<void> {
  return invoke("terminal_close", { sessionId });
}

export async function terminalRename(
  sessionId: string,
  title: string
): Promise<void> {
  return invoke("terminal_rename", { sessionId, title });
}

export async function terminalSetActive(
  repoId: string,
  sessionId: string
): Promise<void> {
  return invoke("terminal_set_active", { repoId, sessionId });
}

export async function terminalGetActive(
  repoId: string
): Promise<string | null> {
  return invoke("terminal_get_active", { repoId });
}

// ── PTY I/O ──

export async function terminalWrite(
  sessionId: string,
  data: number[]
): Promise<void> {
  return invoke("terminal_write", { sessionId, data });
}

export async function terminalResize(
  sessionId: string,
  cols: number,
  rows: number
): Promise<void> {
  return invoke("terminal_resize", { sessionId, cols, rows });
}

export async function terminalSubscribe(
  sessionId: string,
  onOutput: (data: TerminalOutputData) => void
): Promise<void> {
  const channel = new Channel<TerminalOutputData>();
  channel.onmessage = onOutput;
  return invoke("terminal_subscribe", { sessionId, channel });
}

export async function terminalGetForegroundProcess(
  sessionId: string
): Promise<ProcessInfo | null> {
  return invoke("terminal_get_foreground_process", { sessionId });
}

// ── Config ──

export async function configGet(): Promise<UserConfig> {
  return invoke("config_get");
}

export async function configUpdate(
  patch: Partial<UserConfig>
): Promise<UserConfig> {
  return invoke("config_update", { patch });
}

// ── Layout ──

export async function stateSaveLayout(layout: AppLayout): Promise<void> {
  return invoke("state_save_layout", { layout });
}

export async function stateLoadLayout(): Promise<AppLayout | null> {
  return invoke("state_load_layout");
}
