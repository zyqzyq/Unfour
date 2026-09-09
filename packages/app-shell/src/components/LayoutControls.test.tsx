// @vitest-environment jsdom
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { LayoutControls } from "./LayoutControls";

afterEach(cleanup);

describe("LayoutControls", () => {
  it("renders compact shell layout toggles with pressed state", () => {
    render(
      <LayoutControls
        bottomPanelCollapsed={false}
        onToggleBottomPanel={vi.fn()}
        onToggleSidebar={vi.fn()}
        sidebarCollapsed
      />,
    );

    expect(screen.getByRole("button", { name: "Expand sidebar" })).toHaveAttribute(
      "aria-pressed",
      "false",
    );
    expect(screen.getByRole("button", { name: "Toggle bottom panel" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.queryByRole("button", { name: "Toggle inspector" })).toBeNull();
  });

  it("invokes each layout toggle handler", () => {
    const onToggleBottomPanel = vi.fn();
    const onToggleSidebar = vi.fn();

    render(
      <LayoutControls
        bottomPanelCollapsed
        onToggleBottomPanel={onToggleBottomPanel}
        onToggleSidebar={onToggleSidebar}
        sidebarCollapsed={false}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Collapse sidebar" }));
    fireEvent.click(screen.getByRole("button", { name: "Toggle bottom panel" }));

    expect(onToggleSidebar).toHaveBeenCalledTimes(1);
    expect(onToggleBottomPanel).toHaveBeenCalledTimes(1);
  });

  it("only exposes the sidebar control when no module output is available", () => {
    render(<LayoutControls bottomPanelCollapsed onToggleSidebar={vi.fn()} sidebarCollapsed={false} />);
    expect(screen.getByRole("button", { name: "Collapse sidebar" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Toggle bottom panel" })).toBeNull();
  });
});
