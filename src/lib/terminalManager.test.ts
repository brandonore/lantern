import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Terminal } from "@xterm/xterm";

vi.mock("./tauriCommands", () => ({
  terminalGetForegroundProcess: vi.fn().mockResolvedValue(null),
  terminalWrite: vi.fn().mockResolvedValue(undefined),
  terminalResize: vi.fn().mockResolvedValue(undefined),
  terminalSubscribe: vi.fn().mockResolvedValue(undefined),
}));

import {
  terminalGetForegroundProcess,
  terminalResize,
  terminalSubscribe,
  terminalWrite,
} from "./tauriCommands";
import { terminalManager } from "./terminalManager";
import type { TerminalLatencyMode } from "../types";

const mockTerminalGetForegroundProcess =
  terminalGetForegroundProcess as ReturnType<typeof vi.fn>;
const mockTerminalSubscribe = terminalSubscribe as ReturnType<typeof vi.fn>;
const mockTerminalWrite = terminalWrite as ReturnType<typeof vi.fn>;
const mockTerminalResize = terminalResize as ReturnType<typeof vi.fn>;
const terminalCtor = Terminal as unknown as ReturnType<typeof vi.fn>;

function getTerminalMock(index = -1) {
  const mockIndex =
    index >= 0 ? index : terminalCtor.mock.results.length - 1;
  return terminalCtor.mock.results[mockIndex]?.value;
}

function getTerminalCallbacks(index = -1) {
  const terminal = getTerminalMock(index);

  return {
    onData: terminal.onData.mock.calls[0][0] as (data: string) => void,
    onBinary: terminal.onBinary.mock.calls[0][0] as (data: string) => void,
  };
}

function getTerminalOutputHandler(index = -1) {
  const mockIndex =
    index >= 0 ? index : mockTerminalSubscribe.mock.calls.length - 1;
  return mockTerminalSubscribe.mock.calls[mockIndex][1] as (
    output: { kind: "Data"; data: string } | { kind: "Exited"; code: number | null }
  ) => void;
}

async function flushAsyncWork() {
  await Promise.resolve();
  await Promise.resolve();
}

function makeConfig(latencyMode: TerminalLatencyMode = "low-latency") {
  return {
    fontFamily: "JetBrains Mono",
    fontSize: 14,
    scrollback: 1000,
    latencyMode,
  };
}

describe("terminalManager", () => {
  beforeEach(() => {
    vi.useFakeTimers();
    terminalManager.destroyAll();
    terminalCtor.mockClear();
    mockTerminalGetForegroundProcess.mockReset();
    mockTerminalGetForegroundProcess.mockResolvedValue(null);
    mockTerminalSubscribe.mockReset();
    mockTerminalSubscribe.mockResolvedValue(undefined);
    mockTerminalWrite.mockReset();
    mockTerminalWrite.mockResolvedValue(undefined);
  });

  afterEach(() => {
    terminalManager.destroyAll();
    vi.useRealTimers();
  });

  it("batches printable text input before writing to the PTY", async () => {
    await terminalManager.create(
      "tab-1",
      document.createElement("div"),
      makeConfig()
    );

    const { onData } = getTerminalCallbacks();
    onData("a");
    onData("b");

    expect(mockTerminalWrite).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(6);
    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
    const [sessionId, bytes, seq] = mockTerminalWrite.mock.calls[0];
    expect(sessionId).toBe("tab-1");
    expect(Array.from(bytes as Uint8Array)).toEqual([97, 98]);
    expect(seq).toBeUndefined();
  });

  it("writes binary input directly as byte values", async () => {
    await terminalManager.create(
      "tab-2",
      document.createElement("div"),
      makeConfig()
    );

    const { onBinary } = getTerminalCallbacks();
    onBinary("\u0000\u0001");

    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
    const [sessionId, bytes, seq] = mockTerminalWrite.mock.calls[0];
    expect(sessionId).toBe("tab-2");
    expect(Array.from(bytes as Uint8Array)).toEqual([0, 1]);
    expect(seq).toBeUndefined();
  });

  it("batches PTY output while an xterm write is still in flight", async () => {
    await terminalManager.create(
      "tab-output-batch",
      document.createElement("div"),
      makeConfig()
    );

    const terminal = getTerminalMock();
    const onOutput = getTerminalOutputHandler();
    let resolveWrite = () => {};

    terminal.write.mockImplementationOnce((_data: string, callback?: () => void) => {
      resolveWrite = callback ?? (() => {});
    });

    onOutput({ kind: "Data", data: "a" });
    onOutput({ kind: "Data", data: "b" });
    onOutput({ kind: "Data", data: "c" });

    expect(terminal.write).toHaveBeenCalledTimes(1);
    expect(terminal.write.mock.calls[0][0]).toBe("a");

    resolveWrite();

    expect(terminal.write).toHaveBeenCalledTimes(2);
    expect(terminal.write.mock.calls[1][0]).toBe("bc");
  });

  it("flushes queued printable input before submit keys", async () => {
    await terminalManager.create(
      "tab-enter",
      document.createElement("div"),
      makeConfig()
    );

    const { onData } = getTerminalCallbacks();
    onData("a");
    onData("\r");

    expect(mockTerminalWrite).toHaveBeenCalledTimes(2);
    expect(Array.from(mockTerminalWrite.mock.calls[0][1] as Uint8Array)).toEqual([97]);
    expect(Array.from(mockTerminalWrite.mock.calls[1][1] as Uint8Array)).toEqual([13]);
  });

  it("renders predictive echo for direct shell prompt typing", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "bash",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-3",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-3");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");
    onData("b");

    expect(terminal.registerDecoration).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(61);
    expect(terminal.registerDecoration).toHaveBeenCalledTimes(1);
    expect(terminal.registerDecoration.mock.calls[0][0]).toMatchObject({
      x: 0,
      width: 2,
    });
    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
  });

  it("shrinks predictive echo on end-of-line backspace", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "zsh",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-4",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-4");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");
    onData("b");
    await vi.advanceTimersByTimeAsync(61);
    onData("\u007f");
    await vi.advanceTimersByTimeAsync(20);

    expect(terminal.registerDecoration).toHaveBeenCalledTimes(2);
    expect(terminal.registerDecoration.mock.calls[1][0]).toMatchObject({
      x: 0,
      width: 1,
    });
  });

  it("clears predictive echo before backend output is written", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "fish",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-5",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-5");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    const onOutput = getTerminalOutputHandler();

    onData("a");
    await vi.advanceTimersByTimeAsync(61);
    const decoration = terminal.registerDecoration.mock.results[0].value;

    onOutput({ kind: "Data", data: "a" });

    expect(decoration.dispose).toHaveBeenCalledTimes(1);
    expect(terminal.write.mock.calls[0][0]).toBe("a");
  });

  it("does not predict for tmux sessions", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "tmux",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-6",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-6");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");

    expect(terminal.registerDecoration).not.toHaveBeenCalled();
    expect(mockTerminalWrite).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(6);
    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
  });

  it("allows predictive echo for recognized agent UIs in alternate screen", async () => {
    await terminalManager.create(
      "tab-agent",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-agent");

    const terminal = getTerminalMock();
    terminal.buffer.active.type = "alternate";
    const managed = (terminalManager as any).terminals.get("tab-agent");
    managed.predictiveEcho.foregroundProcessPolled = true;
    managed.predictiveEcho.foregroundProcessInfo = {
      name: "codex",
      is_agent: true,
      agent_label: "Codex",
    };

    expect((terminalManager as any).isPredictiveEchoEligible(managed)).toBe(true);
    expect((terminalManager as any).extendPredictiveEcho(managed, "a")).toBe(true);
    expect(managed.predictiveEcho.pendingText).toBe("a");
    expect(managed.predictiveEcho.visible).toBe(true);
    expect(terminal.registerMarker).toHaveBeenCalledTimes(1);
  });

  it("stops foreground-process polling when the active tab is cleared", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "bash",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-7",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-7");
    await flushAsyncWork();

    expect(mockTerminalGetForegroundProcess).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(1100);
    const callCountBeforeClear = mockTerminalGetForegroundProcess.mock.calls.length;
    expect(callCountBeforeClear).toBeGreaterThan(1);

    terminalManager.setActiveTab(null);
    await vi.advanceTimersByTimeAsync(1100);

    expect(mockTerminalGetForegroundProcess).toHaveBeenCalledTimes(
      callCountBeforeClear
    );
  });

  it("renders predictive echo with cursor not at bottom row", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "bash",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-8",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-8");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    // Cursor at row 0 (fresh terminal, not at bottom)
    expect(terminal.buffer.active.cursorY).toBe(0);
    expect(terminal.buffer.active.cursorY).not.toBe(terminal.rows - 1);

    const { onData } = getTerminalCallbacks();
    onData("a");

    await vi.advanceTimersByTimeAsync(61);
    expect(terminal.registerDecoration).toHaveBeenCalledTimes(1);
    expect(terminal.registerDecoration.mock.calls[0][0]).toMatchObject({
      x: 0,
      width: 1,
    });
  });

  it("does not predict when scrolled up from bottom", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "bash",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-9",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-9");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    // Simulate scrolled-up state: viewportY behind baseY
    terminal.buffer.active.baseY = 10;
    terminal.buffer.active.viewportY = 5;

    const { onData } = getTerminalCallbacks();
    onData("a");

    expect(terminal.registerDecoration).not.toHaveBeenCalled();
    expect(mockTerminalWrite).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(6);
    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);

    // Restore for cleanup
    terminal.buffer.active.baseY = 0;
    terminal.buffer.active.viewportY = 0;
  });

  it("predicts optimistically before first foreground process poll completes", async () => {
    // Don't let the poll resolve — keep it pending
    mockTerminalGetForegroundProcess.mockReturnValue(new Promise(() => {}));

    await terminalManager.create(
      "tab-11",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-11");
    // Don't flush — poll is still in flight

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");

    // Should predict even though foreground process is unknown
    await vi.advanceTimersByTimeAsync(61);
    expect(terminal.registerDecoration).toHaveBeenCalledTimes(1);
    expect(terminal.registerDecoration.mock.calls[0][0]).toMatchObject({
      x: 0,
      width: 1,
    });
  });

  it("disables prediction after poll returns non-shell process", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "vim",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-12",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-12");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");

    expect(terminal.registerDecoration).not.toHaveBeenCalled();
    expect(mockTerminalWrite).not.toHaveBeenCalled();
    await vi.advanceTimersByTimeAsync(6);
    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
  });

  it("strips shell integration markers and keeps fallback polling active", async () => {
    mockTerminalGetForegroundProcess.mockResolvedValue({
      name: "bash",
      is_agent: false,
      agent_label: null,
    });

    await terminalManager.create(
      "tab-marker",
      document.createElement("div"),
      makeConfig()
    );
    terminalManager.setActiveTab("tab-marker");
    await flushAsyncWork();

    const onOutput = getTerminalOutputHandler();
    const terminal = getTerminalMock();

    onOutput({
      kind: "Data",
      data: "\u001b]633;Lantern;Prompt\u0007$ ",
    });

    expect(terminal.write.mock.calls[0][0]).toBe("$ ");

    const callsBeforeWait = mockTerminalGetForegroundProcess.mock.calls.length;
    await vi.advanceTimersByTimeAsync(1100);
    expect(mockTerminalGetForegroundProcess.mock.calls.length).toBeGreaterThan(
      callsBeforeWait
    );
  });

  it("sends initial resize after PTY subscribe", async () => {
    mockTerminalResize.mockReset();
    mockTerminalResize.mockResolvedValue(undefined);

    await terminalManager.create(
      "tab-10",
      document.createElement("div"),
      makeConfig()
    );

    const terminal = getTerminalMock();
    // The last call to terminalResize should be the initial sync
    expect(mockTerminalResize).toHaveBeenCalledWith(
      "tab-10",
      terminal.cols,
      terminal.rows
    );
  });
});
