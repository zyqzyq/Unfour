// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { Workspace } from "@unfour/command-client";
import type { DesktopAppExtensionContext } from "../extensions";
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
  };
}

function extensionContext(activeWorkspace: Workspace): DesktopAppExtensionContext {
  return {
    activeTab: { id: "api-main", kind: "api", title: "API Client" },
    activeWorkspace,
    activateWorkspace: vi.fn(),
    refreshWorkspaces: vi.fn(),
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
  it("does not render empty extension groups without extensions", async () => {
    const active = workspace("Default Workspace");
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        extensionContext={extensionContext(active)}
        onActivateWorkspace={vi.fn()}
        workspaces={[active]}
      />,
      { wrapper: createWrapper() },
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });

    expect(await screen.findByText("New workspace")).toBeTruthy();
    expect(screen.getAllByRole("separator")).toHaveLength(1);
  });

  it("keeps the trigger width fixed while workspace names change", () => {
    const first = workspace("Default Workspace");
    const second = workspace("A much longer workspace name");
    const { rerender } = render(
      <WorkspaceMenu
        activeWorkspace={first}
        extensionContext={extensionContext(first)}
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
        extensionContext={extensionContext(second)}
        onActivateWorkspace={vi.fn()}
        workspaces={[first, second]}
      />,
    );

    const secondTrigger = screen.getByRole("button", {
      name: /a much longer workspace name/i,
    });
    expect(secondTrigger).toBe(firstTrigger);
    expect(secondTrigger).toHaveClass("w-[220px]");
    expect(secondTrigger.querySelector("svg")).toHaveClass("ml-auto");
  });

  it("mounts and survives parent rerenders without a maximum update depth error", () => {
    const active = workspace("Default Workspace");
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const onActivateWorkspace = vi.fn();
    const context = extensionContext(active);
    const workspaces = [active];
    const { rerender } = render(
      <div data-render-count={-1}>
        <WorkspaceMenu
          activeWorkspace={active}
          extensionContext={context}
          onActivateWorkspace={onActivateWorkspace}
          workspaces={workspaces}
        />
      </div>,
      { wrapper: createWrapper() },
    );
    const trigger = screen.getByRole("button", { name: /default workspace/i });

    for (let renderCount = 0; renderCount < 10; renderCount += 1) {
      rerender(
        <div data-render-count={renderCount}>
          <WorkspaceMenu
            activeWorkspace={active}
            extensionContext={context}
            onActivateWorkspace={onActivateWorkspace}
            workspaces={workspaces}
          />
        </div>,
      );
      expect(screen.getByRole("button", { name: /default workspace/i })).toBe(trigger);
    }

    expect(
      consoleError.mock.calls.some((call) =>
        call.some((value) => String(value).includes("Maximum update depth exceeded")),
      ),
    ).toBe(false);
    consoleError.mockRestore();
  });

  it("shows environment badges and MCP summaries in the workspace menu", async () => {
    const prod = workspace("Production", "prod");
    const test = workspace("Staging", "test");
    render(
      <WorkspaceMenu
        activeWorkspace={prod}
        extensionContext={extensionContext(prod)}
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
        extensionContext={extensionContext(active)}
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

describe("WorkspaceMenu extensions", () => {
  it("renders extension decorations and runs asynchronous workspace actions", async () => {
    const active = workspace("Default Workspace");
    const run = vi.fn().mockResolvedValue(undefined);
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        decoration={({ placement }) => <span>{`extension-${placement}`}</span>}
        extensionContext={extensionContext(active)}
        onActivateWorkspace={vi.fn()}
        workspaceActions={[
          {
            id: "test.publish",
            label: "Publish workspace",
            run,
          },
        ]}
        workspaces={[active]}
      />,
      { wrapper: createWrapper() },
    );

    expect(screen.getByText("extension-trigger")).toBeTruthy();
    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });
    expect(await screen.findByText("extension-listItem")).toBeTruthy();
    fireEvent.click(screen.getByText("Publish workspace"));
    expect(run).toHaveBeenCalledWith(
      expect.objectContaining({ workspace: active, activeWorkspace: active }),
    );
  });

  it("renders workspace-specific decorations and permits null decorations", async () => {
    const active = workspace("Default Workspace");
    const decorated = workspace("Decorated Workspace");
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        decoration={({ placement, workspace: item }) =>
          item.id === decorated.id ? <span>{`marker-${placement}`}</span> : null
        }
        extensionContext={extensionContext(active)}
        onActivateWorkspace={vi.fn()}
        workspaces={[active, decorated]}
      />,
      { wrapper: createWrapper() },
    );

    expect(screen.queryByText("marker-trigger")).toBeNull();
    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });
    expect(await screen.findByText("marker-listItem")).toBeTruthy();
  });

  it("resolves workspace actions for the active workspace and respects disabled actions", async () => {
    const active = workspace("Default Workspace");
    const runEnabled = vi.fn();
    const runDisabled = vi.fn();
    const provider = vi.fn(() => [
      { id: "test.enabled" as const, label: "Enabled extension action", run: runEnabled },
      {
        id: "test.disabled" as const,
        label: "Disabled extension action",
        disabled: true,
        run: runDisabled,
      },
    ]);
    const context = extensionContext(active);
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        extensionContext={context}
        onActivateWorkspace={vi.fn()}
        workspaceActionProvider={provider}
        workspaces={[active]}
      />,
      { wrapper: createWrapper() },
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByText("Enabled extension action"));
    expect(runEnabled).toHaveBeenCalledWith(
      expect.objectContaining({ activeWorkspace: active, workspace: active }),
    );
    expect(provider).toHaveBeenCalledWith(context, active);

    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByText("Disabled extension action"));
    expect(runDisabled).not.toHaveBeenCalled();
  });

  it("renders footer actions in a final group and runs them with extension context", async () => {
    const active = workspace("Default Workspace");
    const context = extensionContext(active);
    const run = vi.fn();
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        extensionContext={context}
        onActivateWorkspace={vi.fn()}
        workspaceMenuFooterActions={[
          { id: "test.import", label: "Import workspace", run },
        ]}
        workspaces={[active]}
      />,
      { wrapper: createWrapper() },
    );

    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByText("Import workspace"));

    expect(run).toHaveBeenCalledWith(context);
  });

  it("does not show a static disabled reason when an action is disabled only by pending work", async () => {
    const active = workspace("Default Workspace");
    let finish!: () => void;
    const pending = new Promise<void>((resolve) => {
      finish = resolve;
    });
    render(
      <WorkspaceMenu
        activeWorkspace={active}
        extensionContext={extensionContext(active)}
        onActivateWorkspace={vi.fn()}
        workspaceActions={[
          { id: "test.pending", label: "Start publish", run: () => pending },
          {
            id: "test.available",
            label: "Available action",
            disabled: false,
            disabledReason: "Only available for another workspace",
            run: vi.fn(),
          },
        ]}
        workspaces={[active]}
      />,
      { wrapper: createWrapper() },
    );

    const trigger = screen.getByRole("button", { name: /default workspace/i });
    fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false });
    fireEvent.click(await screen.findByText("Start publish"));
    fireEvent.pointerDown(trigger, { button: 0, ctrlKey: false });

    expect(await screen.findByText("Available action")).toBeTruthy();
    expect(screen.queryByText("Only available for another workspace")).toBeNull();
    finish();
    await waitFor(() => expect(screen.getByText("Available action").closest("[role=menuitem]")).not.toHaveAttribute("data-disabled"));
  });
});
