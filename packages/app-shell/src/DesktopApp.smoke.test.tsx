// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DesktopApp } from "./DesktopApp";

const queryMocks = vi.hoisted(() => ({
  invalidateQueries: vi.fn().mockResolvedValue(undefined),
  mutate: vi.fn(),
  setQueryData: vi.fn(),
}));

vi.mock("@tanstack/react-query", () => ({
  useMutation: () => ({ mutate: queryMocks.mutate }),
  useQuery: ({ queryKey }: { queryKey: readonly unknown[] }) => {
    if (queryKey[0] === "system-health") return { data: { storageReady: true } };
    if (queryKey[0] === "workspaces") {
      return {
        data: {
          activeWorkspaceId: "ws-default",
          workspaces: [
            {
              createdAt: "2026-01-01T00:00:00.000Z",
              deletedAt: null,
              environmentType: "dev",
              id: "ws-default",
              isDefault: true,
              lastOpenedAt: null,
              mcpPolicy: "auto",
              name: "Default Workspace",
              revision: 1,
              updatedAt: "2026-01-01T00:00:00.000Z",
            },
          ],
        },
      };
    }
    if (queryKey[0] === "workspace-environments") return { data: [] };
    if (queryKey[0] === "database-connections") return { data: [] };
    return { data: undefined };
  },
  useQueryClient: () => ({
    invalidateQueries: queryMocks.invalidateQueries,
    setQueryData: queryMocks.setQueryData,
  }),
}));

vi.mock("@unfour/command-client", () => ({
  createWorkspace: vi.fn(),
  deleteWorkspace: vi.fn(),
  exportDiagnosticsBundle: vi.fn(),
  getSystemHealth: vi.fn(),
  getWorkspaceLayout: vi.fn(),
  getWorkspaceState: vi.fn(),
  listDatabaseConnections: vi.fn(),
  listWorkspaceEnvironments: vi.fn(),
  openDiagnosticsDir: vi.fn(),
  openLogDir: vi.fn(),
  renameWorkspace: vi.fn(),
  setActiveWorkspace: vi.fn(),
  setActiveWorkspaceEnvironment: vi.fn(),
  updateWorkspaceEnvironment: vi.fn(),
}));

vi.mock("@unfour/workspace-core", () => ({
  useWorkspaceStore: () => ({
    activeTabId: "api-main",
    activeWorkspaceId: "ws-default",
    bottomPanelHeight: 240,
    rightInspectorWidth: 320,
    setActiveTab: vi.fn(),
    setActiveWorkspace: vi.fn(),
    setBottomPanelHeight: vi.fn(),
    setRightInspectorWidth: vi.fn(),
    setSelectedApiRequest: vi.fn(),
    setModuleSidebarWidth: vi.fn(),
    sidebarCollapsed: false,
    sidebarWidths: { api: 320, ssh: 248, database: 280 },
    tabs: [{ id: "api-main", kind: "api", title: "API Client" }],
    toggleSidebar: vi.fn(),
  }),
}));

vi.mock("@unfour/api-client", () => ({ ApiClientPage: () => null }));
vi.mock("@unfour/database", () => ({ DatabasePage: () => null }));
vi.mock("@unfour/ssh-terminal", () => ({
  TerminalLogPanel: () => null,
  TerminalPage: () => null,
  TerminalStatusBar: ({ rightAccessory }: { rightAccessory?: ReactNode }) => (
    <>{rightAccessory}</>
  ),
}));
vi.mock("@unfour/workspace-environments", () => ({
  WorkspaceEnvironmentsPage: () => null,
  WorkspaceEnvironmentsStatusBar: () => null,
}));

vi.mock("./AppShell", () => ({
  default: ({ globalToolbar, main }: { globalToolbar?: ReactNode; main: ReactNode }) => (
    <main>
      {globalToolbar}
      {main}
    </main>
  ),
}));
vi.mock("./components/BottomPanelPlaceholder", () => ({ BottomPanelPlaceholder: () => null }));
vi.mock("./components/LayoutControls", () => ({ LayoutControls: () => null }));
vi.mock("./components/ModuleActivityBar", () => ({ ModuleActivityBar: () => null }));
vi.mock("./components/ModuleSidebar", () => ({ ModuleSidebar: () => null }));
vi.mock("./components/RightInspectorPlaceholder", () => ({ RightInspectorPlaceholder: () => null }));
vi.mock("./components/StatusBarPlaceholder", () => ({ StatusBarPlaceholder: () => null }));
vi.mock("./components/WindowControls", () => ({ WindowControls: () => null }));
vi.mock("./components/settings/SettingsDialog", () => ({ SettingsDialog: () => null }));
vi.mock("./components/useLayoutPersistence", () => ({ useLayoutPersistence: vi.fn() }));
vi.mock("./components/useWorkspaceInit", () => ({ useWorkspaceInit: vi.fn() }));

afterEach(() => {
  cleanup();
  vi.restoreAllMocks();
});

describe("DesktopApp startup smoke", () => {
  it("mounts without extensions and keeps the application root populated", () => {
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});
    const { container } = render(<DesktopApp />);

    expect(container).not.toBeEmptyDOMElement();
    expect(screen.getByRole("button", { name: /default workspace/i })).toBeTruthy();
    expect(
      consoleError.mock.calls.some((call) =>
        call.some((value) => String(value).includes("Maximum update depth exceeded")),
      ),
    ).toBe(false);
  });

  it("renders workspace decoration, actions, and footer actions", async () => {
    render(
      <DesktopApp
        extensions={{
          workspaceActions: [
            { id: "test.publish", label: "Publish workspace", run: vi.fn() },
          ],
          workspaceDecoration: ({ placement }) => <span>{`decoration-${placement}`}</span>,
          workspaceMenuFooterActions: [
            { id: "test.import", label: "Import workspace", run: vi.fn() },
          ],
        }}
      />,
    );

    expect(screen.getByText("decoration-trigger")).toBeTruthy();
    fireEvent.pointerDown(screen.getByRole("button", { name: /default workspace/i }), {
      button: 0,
      ctrlKey: false,
    });

    expect(await screen.findByText("decoration-listItem")).toBeTruthy();
    expect(screen.getByText("Publish workspace")).toBeTruthy();
    expect(screen.getByText("Import workspace")).toBeTruthy();
  });
});
