// @vitest-environment jsdom
import type { ReactNode } from "react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import type { Workspace } from "@unfour/command-client";
import { I18nProvider, ThemeProvider } from "@unfour/ui";
import { AppTitleBar } from "./AppTitleBar";
import type { DesktopAppExtensionContext } from "../extensions";

const { mockWindow } = vi.hoisted(() => ({
  mockWindow: {
    isMaximized: vi.fn().mockResolvedValue(false),
    onResized: vi.fn().mockResolvedValue(vi.fn()),
    minimize: vi.fn(),
    toggleMaximize: vi.fn().mockResolvedValue(undefined),
    startDragging: vi.fn().mockResolvedValue(undefined),
    close: vi.fn(),
  },
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => mockWindow,
}));

afterEach(cleanup);

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
    syncStatus: "local",
    remoteId: null,
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
  };
}

describe("AppTitleBar settings entry", () => {
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
});
