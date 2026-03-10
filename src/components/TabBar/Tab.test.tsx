import { describe, it, expect, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { Tab } from "./Tab";
import type { TerminalTab } from "../../types";

const makeTab = (): TerminalTab => ({
  id: "t1",
  repoId: "r1",
  name: "Terminal 1",
  shell: null,
  sortOrder: 0,
});

describe("Tab", () => {
  it("displays tab name", () => {
    render(
      <Tab
        tab={makeTab()}
        isActive={false}
        onClick={vi.fn()}
        onClose={vi.fn()}
        onRename={vi.fn()}
      />
    );
    expect(screen.getByText("Terminal 1")).toBeDefined();
  });

  it("enters rename mode on double-click", () => {
    render(
      <Tab
        tab={makeTab()}
        isActive={true}
        onClick={vi.fn()}
        onClose={vi.fn()}
        onRename={vi.fn()}
      />
    );
    const tabEl = screen.getByText("Terminal 1").closest("[class*='tab']")!;
    fireEvent.doubleClick(tabEl);
    const input = screen.getByDisplayValue("Terminal 1");
    expect(input).toBeDefined();
  });

  it("saves rename on Enter", () => {
    const onRename = vi.fn();
    render(
      <Tab
        tab={makeTab()}
        isActive={true}
        onClick={vi.fn()}
        onClose={vi.fn()}
        onRename={onRename}
      />
    );
    const tabEl = screen.getByText("Terminal 1").closest("[class*='tab']")!;
    fireEvent.doubleClick(tabEl);
    const input = screen.getByDisplayValue("Terminal 1");
    fireEvent.change(input, { target: { value: "My Custom Tab" } });
    fireEvent.keyDown(input, { key: "Enter" });
    expect(onRename).toHaveBeenCalledWith("My Custom Tab");
  });

  it("cancels rename on Escape", () => {
    const onRename = vi.fn();
    render(
      <Tab
        tab={makeTab()}
        isActive={true}
        onClick={vi.fn()}
        onClose={vi.fn()}
        onRename={onRename}
      />
    );
    const tabEl = screen.getByText("Terminal 1").closest("[class*='tab']")!;
    fireEvent.doubleClick(tabEl);
    const input = screen.getByDisplayValue("Terminal 1");
    fireEvent.change(input, { target: { value: "Changed" } });
    fireEvent.keyDown(input, { key: "Escape" });
    expect(onRename).not.toHaveBeenCalled();
    // Should show original name again
    expect(screen.getByText("Terminal 1")).toBeDefined();
  });

  it("shows close button on hover", () => {
    const { container } = render(
      <Tab
        tab={makeTab()}
        isActive={false}
        onClick={vi.fn()}
        onClose={vi.fn()}
        onRename={vi.fn()}
      />
    );
    const closeBtn = container.querySelector("[class*='closeButton']");
    expect(closeBtn).not.toBeNull();
  });
});
