import { act, render, screen } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { useAppStore } from "../../stores/appStore";
import { TerminalInstance } from "./TerminalInstance";

const terminalManagerMock = vi.hoisted(() => ({
  create: vi.fn(),
  fitAndFocus: vi.fn(),
  has: vi.fn(),
  hasReceivedOutput: vi.fn(),
}));

vi.mock("../../lib/terminalManager", () => ({
  terminalManager: {
    create: terminalManagerMock.create,
    fitAndFocus: terminalManagerMock.fitAndFocus,
    has: terminalManagerMock.has,
    hasReceivedOutput: terminalManagerMock.hasReceivedOutput,
  },
}));

describe("TerminalInstance", () => {
  beforeEach(() => {
    terminalManagerMock.create.mockReset();
    terminalManagerMock.fitAndFocus.mockReset();
    terminalManagerMock.has.mockReset();
    terminalManagerMock.has.mockReturnValue(false);
    terminalManagerMock.hasReceivedOutput.mockReset();
    terminalManagerMock.hasReceivedOutput.mockReturnValue(false);

    useAppStore.setState({
      config: {
        default_shell: "/bin/bash",
        font_family: "JetBrains Mono",
        font_size: 14,
        scrollback_lines: 10000,
        theme: "dark",
        git_poll_interval_secs: 5,
        ui_scale: 1,
        terminal_latency_mode: "low-latency",
      },
    });
  });

  it("shows a startup placeholder until the first PTY output arrives", () => {
    render(<TerminalInstance tabId="tab-1" isVisible={true} />);

    expect(screen.getByText("Starting shell...")).toBeDefined();
    expect(terminalManagerMock.create).toHaveBeenCalledTimes(1);
  });

  it("hides the startup placeholder after the first PTY output", () => {
    let handleFirstOutput: (() => void) | undefined;
    terminalManagerMock.create.mockImplementation(
      (
        _tabId: string,
        _container: HTMLElement,
        _config: unknown,
        _onExit?: (code: number | null) => void,
        onFirstOutput?: () => void
      ) => {
        handleFirstOutput = onFirstOutput;
      }
    );

    render(<TerminalInstance tabId="tab-2" isVisible={true} />);
    expect(screen.getByText("Starting shell...")).toBeDefined();

    act(() => {
      handleFirstOutput?.();
    });

    expect(screen.queryByText("Starting shell...")).toBeNull();
  });
});
