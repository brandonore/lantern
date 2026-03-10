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
  terminalSubscribe,
  terminalWrite,
} from "./tauriCommands";
import { terminalManager } from "./terminalManager";

const mockTerminalGetForegroundProcess =
  terminalGetForegroundProcess as ReturnType<typeof vi.fn>;
const mockTerminalSubscribe = terminalSubscribe as ReturnType<typeof vi.fn>;
const mockTerminalWrite = terminalWrite as ReturnType<typeof vi.fn>;
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

  it("writes text input directly as terminal bytes", async () => {
    await terminalManager.create(
      "tab-1",
      document.createElement("div"),
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );

    const { onData } = getTerminalCallbacks();
    onData("ab");

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
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );

    const { onBinary } = getTerminalCallbacks();
    onBinary("\u0000\u0001");

    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
    const [sessionId, bytes, seq] = mockTerminalWrite.mock.calls[0];
    expect(sessionId).toBe("tab-2");
    expect(Array.from(bytes as Uint8Array)).toEqual([0, 1]);
    expect(seq).toBeUndefined();
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
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );
    terminalManager.setActiveTab("tab-3");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");
    onData("b");

    expect(terminal.registerDecoration).toHaveBeenCalledTimes(2);
    expect(terminal.registerDecoration.mock.calls[0][0]).toMatchObject({
      x: 0,
      width: 1,
    });
    expect(terminal.registerDecoration.mock.calls[1][0]).toMatchObject({
      x: 0,
      width: 2,
    });
    expect(mockTerminalWrite).toHaveBeenCalledTimes(2);
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
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );
    terminalManager.setActiveTab("tab-4");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");
    onData("b");
    onData("\u007f");

    expect(terminal.registerDecoration).toHaveBeenCalledTimes(3);
    expect(terminal.registerDecoration.mock.calls[2][0]).toMatchObject({
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
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );
    terminalManager.setActiveTab("tab-5");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    const onOutput = getTerminalOutputHandler();

    onData("a");
    const decoration = terminal.registerDecoration.mock.results[0].value;

    onOutput({ kind: "Data", data: "a" });

    expect(decoration.dispose).toHaveBeenCalledTimes(1);
    expect(terminal.write).toHaveBeenCalledWith("a");
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
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );
    terminalManager.setActiveTab("tab-6");
    await flushAsyncWork();

    const terminal = getTerminalMock();
    const { onData } = getTerminalCallbacks();
    onData("a");

    expect(terminal.registerDecoration).not.toHaveBeenCalled();
    expect(mockTerminalWrite).toHaveBeenCalledTimes(1);
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
      { fontFamily: "JetBrains Mono", fontSize: 14, scrollback: 1000 }
    );
    terminalManager.setActiveTab("tab-7");
    await flushAsyncWork();

    expect(mockTerminalGetForegroundProcess).toHaveBeenCalledTimes(1);

    await vi.advanceTimersByTimeAsync(450);
    const callCountBeforeClear = mockTerminalGetForegroundProcess.mock.calls.length;
    expect(callCountBeforeClear).toBeGreaterThan(1);

    terminalManager.setActiveTab(null);
    await vi.advanceTimersByTimeAsync(450);

    expect(mockTerminalGetForegroundProcess).toHaveBeenCalledTimes(
      callCountBeforeClear
    );
  });
});
