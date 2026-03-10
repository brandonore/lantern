import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import { useAppStore } from "../../stores/appStore";
import { StatusBar } from "./StatusBar";

// Mock the agent detector hook
vi.mock("../../hooks/useAgentDetector", () => ({
  useAgentDetector: vi.fn().mockReturnValue(null),
}));

// Mock terminalManager
vi.mock("../../lib/terminalManager", () => ({
  terminalManager: {
    getDimensions: vi.fn().mockReturnValue({ cols: 80, rows: 24 }),
  },
}));

import { useAgentDetector } from "../../hooks/useAgentDetector";
const mockUseAgentDetector = useAgentDetector as ReturnType<typeof vi.fn>;

beforeEach(() => {
  useAppStore.setState({
    repos: [
      {
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
        tabs: [
          {
            id: "t1",
            repoId: "r1",
            name: "Terminal 1",
            shell: null,
            sortOrder: 0,
          },
        ],
        activeTabId: "t1",
      },
    ],
    activeRepoId: "r1",
  });
});

describe("StatusBar", () => {
  it("displays current working directory", () => {
    render(<StatusBar />);
    expect(screen.getByText("/home/user/my-repo")).toBeDefined();
  });

  it("displays terminal dimensions", () => {
    render(<StatusBar />);
    expect(screen.getByText("80x24")).toBeDefined();
  });

  it("displays agent name when agent is running", () => {
    mockUseAgentDetector.mockReturnValue({
      name: "claude",
      is_agent: true,
      agent_label: "Claude Code",
    });
    render(<StatusBar />);
    expect(screen.getByText("Claude Code")).toBeDefined();
  });

  it("shows no agent indicator when plain shell", () => {
    mockUseAgentDetector.mockReturnValue({
      name: "bash",
      is_agent: false,
      agent_label: null,
    });
    render(<StatusBar />);
    expect(screen.queryByText("Claude Code")).toBeNull();
    expect(screen.getByText("bash")).toBeDefined();
  });
});
