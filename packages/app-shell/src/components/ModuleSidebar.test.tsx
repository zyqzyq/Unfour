// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { WorkspaceSidebarWidths, WorkspaceTab } from "@unfour/command-client";
import { MODULE_SIDEBAR_CONFIG } from "@unfour/workspace-core";
import { ModuleSidebar } from "./ModuleSidebar";

type SidebarMockProps = {
  children: ReactNode;
  maxWidth?: number;
  minWidth?: number;
  onWidthChange?: (width: number) => void;
  width?: number;
};

vi.mock("@unfour/ui", () => ({
  Sidebar: ({ children, maxWidth, minWidth, onWidthChange, width }: SidebarMockProps) => (
    <aside
      data-max-width={maxWidth}
      data-min-width={minWidth}
      data-testid="module-sidebar"
      data-width={width}
    >
      <button onClick={() => onWidthChange?.((width ?? 0) + 1)} type="button">
        Resize sidebar
      </button>
      {children}
    </aside>
  ),
}));

afterEach(cleanup);

const sidebarWidths: WorkspaceSidebarWidths = {
  api: 500,
  ssh: 250,
  database: 360,
};

function tabFor(kind: WorkspaceTab["kind"]): WorkspaceTab {
  return { id: `${kind}-main`, kind, title: kind };
}

function renderModuleSidebar(kind: WorkspaceTab["kind"], onWidthChange = vi.fn()) {
  render(
    <ModuleSidebar
      activeTab={tabFor(kind)}
      apiSidebarContent={<span>API content</span>}
      collapsed={false}
      databaseSidebarContent={<span>Database content</span>}
      onModuleWidthChange={onWidthChange}
      sidebarWidths={sidebarWidths}
      sshSidebarContent={<span>SSH content</span>}
    />,
  );
  return onWidthChange;
}

describe("ModuleSidebar", () => {
  it.each([
    ["api", "API content"],
    ["ssh", "SSH content"],
    ["database", "Database content"],
  ] as const)("uses the %s module width and bounds", (kind, content) => {
    renderModuleSidebar(kind);
    const sidebar = screen.getByTestId("module-sidebar");
    const config = MODULE_SIDEBAR_CONFIG[kind];

    expect(sidebar.getAttribute("data-width")).toBe(String(sidebarWidths[kind]));
    expect(sidebar.getAttribute("data-min-width")).toBe(String(config.minWidth));
    expect(sidebar.getAttribute("data-max-width")).toBe(String(config.maxWidth));
    expect(screen.getByText(content)).toBeTruthy();
  });

  it("routes a resize to the active module only", () => {
    const onWidthChange = renderModuleSidebar("api");

    fireEvent.click(screen.getByRole("button", { name: "Resize sidebar" }));

    expect(onWidthChange).toHaveBeenCalledWith("api", 501);
  });

  it("restores each module width when the active module changes", () => {
    const onWidthChange = vi.fn();
    const { rerender } = render(
      <ModuleSidebar
        activeTab={tabFor("api")}
        collapsed={false}
        onModuleWidthChange={onWidthChange}
        sidebarWidths={sidebarWidths}
      />,
    );

    expect(screen.getByTestId("module-sidebar").getAttribute("data-width")).toBe("500");
    rerender(
      <ModuleSidebar
        activeTab={tabFor("ssh")}
        collapsed={false}
        onModuleWidthChange={onWidthChange}
        sidebarWidths={sidebarWidths}
      />,
    );

    expect(screen.getByTestId("module-sidebar").getAttribute("data-width")).toBe("250");
  });
});
