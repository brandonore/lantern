import { beforeEach, describe, expect, it, vi } from "vitest";
import { fireEvent, render, screen } from "@testing-library/react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useAppStore } from "../stores/appStore";
import { TitleBar } from "./TitleBar";

describe("TitleBar", () => {
  beforeEach(() => {
    const appWindow = getCurrentWindow() as any;
    appWindow.startDragging.mockClear();

    useAppStore.setState({
      sidebarCollapsed: false,
    });
  });

  it("toggles the sidebar and updates the accessible label", () => {
    render(<TitleBar />);

    const toggleButton = screen.getByRole("button", { name: "Hide sidebar" });
    fireEvent.click(toggleButton);

    expect(useAppStore.getState().sidebarCollapsed).toBe(true);
    expect(screen.getByRole("button", { name: "Show sidebar" })).toBeDefined();
  });

  it("does not start window dragging from the sidebar toggle", () => {
    const appWindow = getCurrentWindow() as any;
    render(<TitleBar />);

    fireEvent.mouseDown(screen.getByRole("button", { name: "Hide sidebar" }), {
      button: 0,
    });

    expect(appWindow.startDragging).not.toHaveBeenCalled();
  });
});
