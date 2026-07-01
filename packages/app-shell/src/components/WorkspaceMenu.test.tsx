// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import type { Workspace } from "@unfour/command-client";
import { WorkspaceMenu } from "./WorkspaceMenu";

afterEach(cleanup);

function workspace(
  name: string,
  environmentType: Workspace["environmentType"] = "dev",
  mcpPolicy: Workspace["mcpPolicy"] = "auto",
): Workspace {
  return {
    id: `ws-${name}`,
    name,
    environmentType,
    mcpPolicy,
    isDefault: false,
    lastOpenedAt: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    deletedAt: null,
    revision: 1,
    syncStatus: "local",
    remoteId: null,
  };
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>;
  };
}

describe("WorkspaceMenu", () => {
  it("keeps the trigger width fixed while workspace names change", () => {
    const first = workspace("Default Workspace");
    const second = workspace("A much longer workspace name");
    const { rerender } = render(
      <WorkspaceMenu
        activeWorkspace={first}
        onActivateWorkspace={vi.fn()}
        workspaces={[first, second]}
      />,
      { wrapper: createWrapper() },
    );

    const firstTrigger = screen.getByRole("button", { name: /default workspace/i });
    expect(firstTrigger).toHaveClass("w-[220px]");
    expect(firstTrigger.querySelector("svg")).toHaveClass("ml-auto");

    rerender(
      <WorkspaceMenu
        activeWorkspace={second}
        onActivateWorkspace={vi.fn()}
        workspaces={[first, second]}
      />,
    );

    const secondTrigger = screen.getByRole("button", {
      name: /a much longer workspace name/i,
    });
    expect(secondTrigger).toHaveClass("w-[220px]");
    expect(secondTrigger.querySelector("svg")).toHaveClass("ml-auto");
  });

  it("shows environment badges and MCP summaries in the workspace menu", async () => {
    const prod = workspace("Production", "prod");
    const test = workspace("Staging", "test");
    render(
      <WorkspaceMenu
        activeWorkspace={prod}
        onActivateWorkspace={vi.fn()}
        workspaces={[prod, test]}
      />,
      { wrapper: createWrapper() },
    );

    expect(screen.getByText("PROD")).toBeTruthy();

    fireEvent.pointerDown(screen.getByRole("button", { name: /production/i }), {
      button: 0,
      ctrlKey: false,
    });

    expect(await screen.findByText("MCP: Read-only")).toBeTruthy();
    expect(screen.getByText("TEST")).toBeTruthy();
    expect(screen.getByText("MCP: Guarded")).toBeTruthy();
  });

  it("offers environment selection when creating a workspace", async () => {
    const active = workspace("Default Workspace");
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        onActivateWorkspace={vi.fn()}
        workspaces={[active]}
      />,
      { wrapper: createWrapper() },
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByText("New workspace"));

    const environmentSelect = screen.getByRole("combobox") as HTMLSelectElement;
    expect(environmentSelect.value).toBe("dev");
    fireEvent.change(environmentSelect, { target: { value: "prod" } });
    expect(environmentSelect.value).toBe("prod");
    expect(
      screen.getByText("Environment controls the default MCP permission level."),
    ).toBeTruthy();
  });
});
