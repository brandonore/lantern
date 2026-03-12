import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { useAppStore } from "../../stores/appStore";
import { SettingsDialog } from "./SettingsDialog";

beforeEach(() => {
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
    settingsOpen: true,
  });
});

describe("SettingsDialog", () => {
  it("renders current settings values", () => {
    render(<SettingsDialog />);
    expect(screen.getByDisplayValue("/bin/bash")).toBeDefined();
    expect(screen.getByDisplayValue("JetBrains Mono")).toBeDefined();
    expect(screen.getByDisplayValue("14")).toBeDefined();
    expect(screen.getByDisplayValue("10000")).toBeDefined();
  });

  it("updates font size", () => {
    render(<SettingsDialog />);
    const input = screen.getByDisplayValue("14");
    fireEvent.change(input, { target: { value: "18" } });
    expect(screen.getByDisplayValue("18")).toBeDefined();
  });

  it("reverts on cancel", () => {
    const setSettingsOpen = vi.fn();
    useAppStore.setState({ setSettingsOpen });
    render(<SettingsDialog />);
    fireEvent.click(screen.getByText("Cancel"));
    expect(setSettingsOpen).toHaveBeenCalledWith(false);
  });
});
