// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import type { Workspace, WorkspaceEnvironment } from "@unfour/command-client";
import { I18nProvider, ThemeProvider } from "@unfour/ui";
import { AppTitleBar } from "./AppTitleBar";
import type { DesktopAppExtensionContext } from "../extensions";

const { mockWindow } = vi.hoisted(() => ({
  mockWindow: {
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(vi.fn()),
    show: vi.fn().mockResolvedValue(undefined),
    minimize: vi.fn(),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    startDragging: vi.fn().mockResolvedValue(undefined),
    close: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: vi.fn(() => ({ ...mockWindow })),
}));

vi.mock("./module-helpers", () => ({
  isTauriRuntime: () => true,
}));

afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

function workspace(): Workspace {
  return {
    id: "ws-default",
    name: "Default Workspace",
    environmentType: "dev",
    mcpPolicy: "auto",
    isDefault: true,
    lastOpenedAt: null,
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    deletedAt: null,
    revision: 1,
  };
}

function createWrapper() {
  const client = new QueryClient({
    defaultOptions: { mutations: { retry: false }, queries: { retry: false } },
  });
  return function Wrapper({ children }: { children: ReactNode }) {
    return (
      <QueryClientProvider client={client}>
        <ThemeProvider defaultThemeMode="dark" storageKey="test.theme">
          <I18nProvider initialLocale="en" storageKey="test.locale">
            {children}
          </I18nProvider>
        </ThemeProvider>
      </QueryClientProvider>
    );
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

function environment(id: string, name: string, isActive = false): WorkspaceEnvironment {
  return {
    id,
    workspaceId: "ws-default",
    name,
    sortOrder: 0,
    isActive,
    variables: [],
    createdAt: "2026-01-01T00:00:00.000Z",
    updatedAt: "2026-01-01T00:00:00.000Z",
    deletedAt: null,
    revision: 1,
  };
}

describe("AppTitleBar settings entry", () => {
  it("registers the window resize listener only once across title-bar rerenders", async () => {
    mockWindow.onResized.mockClear();
    mockWindow.show.mockClear();
    const activeWorkspace = workspace();
    const { rerender } = render(
      <AppTitleBar
        activeWorkspace={activeWorkspace}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        workspaces={[activeWorkspace]}
      />,
      { wrapper: createWrapper() },
    );
    await waitFor(() => expect(mockWindow.onResized).toHaveBeenCalledTimes(1));
    expect(mockWindow.show).toHaveBeenCalled();

    rerender(
      <AppTitleBar
        activeWorkspace={{ ...activeWorkspace, name: "Renamed Workspace" }}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        workspaces={[activeWorkspace]}
      />,
    );

    expect(mockWindow.onResized).toHaveBeenCalledTimes(1);
  });

  it("keeps language and theme controls inside Settings instead of the title bar", () => {
    const activeWorkspace = workspace();
    render(
      <AppTitleBar
        activeWorkspace={activeWorkspace}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        workspaces={[activeWorkspace]}
      />,
      { wrapper: createWrapper() },
    );

    expect(screen.getByRole("button", { name: "Settings" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Theme" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Language" })).toBeNull();
  });

  it("opens Settings from the title bar and updates shared preferences", async () => {
    const activeWorkspace = workspace();
    render(
      <AppTitleBar
        activeWorkspace={activeWorkspace}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        workspaces={[activeWorkspace]}
      />,
      { wrapper: createWrapper() },
    );

    fireEvent.click(screen.getByRole("button", { name: "Settings" }));

    const dialog = await screen.findByRole("dialog", { name: "Settings" });
    expect(dialog).toBeTruthy();

    fireEvent.change(within(dialog).getByLabelText("Theme"), { target: { value: "light" } });
    expect(document.documentElement).toHaveAttribute("data-theme", "light");

    fireEvent.change(within(dialog).getByLabelText("Language"), {
      target: { value: "zh-CN" },
    });
    expect(await screen.findByRole("dialog", { name: "设置" })).toBeTruthy();
    expect(screen.getByRole("button", { hidden: true, name: "设置" })).toBeTruthy();
  });

  it("keeps a short pointer guard after closing Settings with the close button", async () => {
    const activeWorkspace = workspace();
    render(
      <AppTitleBar
        activeWorkspace={activeWorkspace}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        workspaces={[activeWorkspace]}
      />,
      { wrapper: createWrapper() },
    );
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const close = screen.getByRole("button", { name: "Close settings" });
    vi.useFakeTimers();
    fireEvent.pointerDown(close);
    fireEvent.click(close);

    expect(screen.getByTestId("settings-close-guard")).toBeTruthy();
    act(() => vi.runAllTimers());
    expect(screen.queryByTestId("settings-close-guard")).toBeNull();
  });

  it("places an end accessory before window controls and mounts extension settings", async () => {
    const activeWorkspace = workspace();
    render(
      <AppTitleBar
        activeWorkspace={activeWorkspace}
        endAccessory={<button type="button">Edition account</button>}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        settingsSections={[
          {
            component: ({ activeWorkspace: workspaceFromContext }) => (
              <div>Account for {workspaceFromContext?.name}</div>
            ),
            id: "edition.account",
            label: "Account",
          },
        ]}
        workspaces={[activeWorkspace]}
      />,
      { wrapper: createWrapper() },
    );

    expect(screen.getByRole("button", { name: "Edition account" })).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "Settings" }));
    const dialog = await screen.findByRole("dialog", { name: "Settings" });
    fireEvent.click(within(dialog).getByRole("button", { name: "Account" }));

    expect(within(dialog).getByText("Account for Default Workspace")).toBeTruthy();
  });

  it("switches the active workspace environment and opens variable management", async () => {
    const activeWorkspace = workspace();
    const onSelectEnvironment = vi.fn();
    const onManageVariables = vi.fn();
    const onOpenEnvironmentMenu = vi.fn();
    render(
      <AppTitleBar
        activeEnvironmentId="env-dev"
        activeWorkspace={activeWorkspace}
        environments={[
          environment("env-dev", "Development", true),
          environment("env-test", "Test"),
        ]}
        extensionContext={extensionContext(activeWorkspace)}
        onActivateWorkspace={vi.fn()}
        onManageVariables={onManageVariables}
        onOpenEnvironmentMenu={onOpenEnvironmentMenu}
        onSelectEnvironment={onSelectEnvironment}
        workspaces={[activeWorkspace]}
      />,
      { wrapper: createWrapper() },
    );

    const environmentTrigger = screen.getByRole("button", {
      name: "Active environment",
    });
    expect(environmentTrigger).toHaveTextContent("Development");
    expect(environmentTrigger).not.toHaveTextContent("Environment:");

    fireEvent.pointerDown(environmentTrigger, {
      button: 0,
      ctrlKey: false,
    });
    expect(onOpenEnvironmentMenu).toHaveBeenCalledTimes(1);
    fireEvent.click(await screen.findByRole("menuitem", { name: "Test" }));
    expect(onSelectEnvironment).toHaveBeenCalledWith("env-test");

    fireEvent.pointerDown(screen.getByRole("button", { name: "Active environment" }), {
      button: 0,
      ctrlKey: false,
    });
    fireEvent.click(await screen.findByRole("menuitem", { name: "Manage variables" }));
    expect(onManageVariables).toHaveBeenCalledTimes(1);
  });
});
