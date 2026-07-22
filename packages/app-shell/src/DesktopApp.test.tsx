// @vitest-environment jsdom
import type { ReactNode } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { DesktopApp } from "./DesktopApp";
import type {
  DesktopAppExtensionContext,
  DesktopAppSettingsSection,
} from "./extensions";

vi.mock("@tanstack/react-query", () => ({
  useMutation: () => ({ mutate: vi.fn() }),
  useQuery: ({ queryKey }: { queryKey: readonly unknown[] }) => {
    if (queryKey[0] === "system-health") {
      return { data: { storageReady: true, syncStrategy: "local-first" } };
    }
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
              remoteId: null,
              revision: 1,
              syncStatus: "local",
              updatedAt: "2026-01-01T00:00:00.000Z",
            },
          ],
        },
      };
    }
    if (queryKey[0] === "database-connections") {
      return { data: [] };
    }
    if (queryKey[0] === "workspace-environments") {
      return {
        data: [
          {
            id: "env-dev",
            workspaceId: "ws-default",
            name: "Development",
            isActive: true,
            variables: [],
          },
        ],
      };
    }
    return { data: undefined };
  },
  useQueryClient: () => ({ invalidateQueries: vi.fn() }),
}));

vi.mock("@unfour/command-client", () => ({
  exportDiagnosticsBundle: vi.fn(),
  getSystemHealth: vi.fn(),
  getWorkspaceLayout: vi.fn(),
  getWorkspaceState: vi.fn(),
  listDatabaseConnections: vi.fn(),
  openDiagnosticsDir: vi.fn(),
  openLogDir: vi.fn(),
  setActiveWorkspace: vi.fn(),
}));

const setActiveTab = vi.fn();

vi.mock("@unfour/workspace-core", () => ({
  useWorkspaceStore: () => ({
    activeTabId: "api-main",
    activeWorkspaceId: "ws-default",
    bottomPanelHeight: 240,
    rightInspectorWidth: 320,
    setActiveTab,
    setActiveWorkspace: vi.fn(),
    setBottomPanelHeight: vi.fn(),
    setRightInspectorWidth: vi.fn(),
    setSelectedApiRequest: vi.fn(),
    setSidebarWidth: vi.fn(),
    sidebarCollapsed: false,
    sidebarWidth: 260,
    tabs: [{ id: "api-main", kind: "api", title: "API Client" }],
    toggleSidebar: vi.fn(),
  }),
}));

vi.mock("@unfour/ui", () => ({
  CommandPalette: ({ actions, open }: { actions: ReactNode; open: boolean }) =>
    open ? <div aria-label="Command palette">{actions}</div> : null,
  ConfirmDialog: ({
    confirmLabel,
    onConfirm,
    onOpenChange,
    open,
    title,
  }: {
    confirmLabel: string;
    onConfirm: () => void;
    onOpenChange: (open: boolean) => void;
    open: boolean;
    title: string;
  }) =>
    open ? (
      <div aria-label={title} role="dialog">
        <button onClick={onConfirm} type="button">
          {confirmLabel}
        </button>
        <button onClick={() => onOpenChange(false)} type="button">
          Cancel
        </button>
      </div>
    ) : null,
  FeedbackProvider: ({ children }: { children: ReactNode }) => <>{children}</>,
  MainWorkspace: ({ children }: { children: ReactNode }) => <>{children}</>,
  useFeedbackErrorHandler: () => vi.fn(),
  useI18n: () => ({ t: (key: string) => key }),
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
  WorkspaceEnvironmentsPage: ({
    initialEnvironmentId,
    onDirtyChange,
  }: {
    initialEnvironmentId?: string | null;
    onDirtyChange?: (dirty: boolean) => void;
  }) => (
    <div>
      Workspace manager: {initialEnvironmentId}
      <button onClick={() => onDirtyChange?.(true)} type="button">
        Mark variables dirty
      </button>
    </div>
  ),
  WorkspaceEnvironmentsStatusBar: ({ workspaceName }: { workspaceName: string }) => (
    <div>Workspace status: {workspaceName}</div>
  ),
}));

vi.mock("./AppShell", () => ({
  default: ({
    activityBar,
    globalToolbar,
    main,
    statusBar,
  }: {
    activityBar?: ReactNode;
    globalToolbar?: ReactNode;
    main: ReactNode;
    statusBar?: ReactNode;
  }) => (
    <>
      {activityBar}
      {globalToolbar}
      {main}
      {statusBar}
    </>
  ),
}));

vi.mock("./components/AppTitleBar", () => ({
  AppTitleBar: ({
    endAccessory,
    extensionContext,
    onManageVariables,
    settingsSections = [],
  }: {
    endAccessory?: ReactNode;
    extensionContext: DesktopAppExtensionContext;
    onManageVariables?: () => void;
    settingsSections?: readonly DesktopAppSettingsSection[];
  }) => (
    <>
      {endAccessory}
      <button onClick={onManageVariables} type="button">
        Manage variables
      </button>
      {settingsSections.map(({ component: Section, id }) => (
        <Section key={id} {...extensionContext} />
      ))}
    </>
  ),
}));
vi.mock("./components/BottomPanelPlaceholder", () => ({ BottomPanelPlaceholder: () => null }));
vi.mock("./components/LayoutControls", () => ({ LayoutControls: () => <span>Layout controls</span> }));
vi.mock("./components/ModuleActivityBar", () => ({
  ModuleActivityBar: ({
    onOpenCommandPalette,
    onSelect,
    onToggleSidebar,
  }: {
    onOpenCommandPalette: () => void;
    onSelect: (tabId: string) => void;
    onToggleSidebar: () => void;
  }) => (
    <>
      <button onClick={onOpenCommandPalette} type="button">
        Open command palette
      </button>
      <button onClick={() => onSelect("ssh-main")} type="button">
        Open SSH Terminal
      </button>
      <button onClick={onToggleSidebar} type="button">
        Toggle module sidebar
      </button>
    </>
  ),
}));
vi.mock("./components/ModuleSidebar", () => ({ ModuleSidebar: () => null }));
vi.mock("./components/RightInspectorPlaceholder", () => ({ RightInspectorPlaceholder: () => null }));
vi.mock("./components/StatusBarPlaceholder", () => ({
  StatusBarPlaceholder: ({ rightAccessory }: { rightAccessory?: ReactNode }) => (
    <>{rightAccessory}</>
  ),
}));
vi.mock("./components/useLayoutPersistence", () => ({ useLayoutPersistence: vi.fn() }));
vi.mock("./components/useWorkspaceInit", () => ({ useWorkspaceInit: vi.fn() }));

afterEach(cleanup);

describe("DesktopApp extensions", () => {
  it("renders every extension surface with readonly shell context", () => {
    const commandRun = vi.fn();
    const observedContexts: DesktopAppExtensionContext[] = [];
    const observe = (label: string) => (context: DesktopAppExtensionContext) => {
      observedContexts.push(context);
      return <span>{label}</span>;
    };

    render(
      <DesktopApp
        extensions={{
          commandPaletteActions: [
            { id: "edition.sync", label: "Edition command", run: commandRun },
          ],
          overlays: observe("Edition overlay"),
          settingsSections: [
            {
              component: observe("Edition settings"),
              id: "edition.account",
              label: "Account",
            },
          ],
          statusBarEnd: observe("Edition status"),
          titleBarEnd: observe("Edition title"),
        }}
      />,
    );

    expect(screen.getByText("Edition title")).toBeTruthy();
    expect(screen.getByText("Edition status")).toBeTruthy();
    expect(screen.getByText("Edition settings")).toBeTruthy();
    expect(screen.getByText("Edition overlay")).toBeTruthy();
    for (const context of observedContexts) {
      expect(Object.keys(context).sort()).toEqual(["activeTab", "activeWorkspace"]);
      expect(context.activeTab).toMatchObject({ id: "api-main", kind: "api" });
      expect(context.activeWorkspace).toMatchObject({ id: "ws-default", name: "Default Workspace" });
    }

    fireEvent.click(screen.getByRole("button", { name: "Open command palette" }));
    fireEvent.click(screen.getByRole("button", { name: "Edition command" }));

    expect(commandRun).toHaveBeenCalledTimes(1);
    expect(commandRun).toHaveBeenCalledWith(
      expect.objectContaining({
        activeTab: expect.objectContaining({ id: "api-main" }),
        activeWorkspace: expect.objectContaining({ id: "ws-default" }),
      }),
    );
    expect(screen.queryByLabelText("Command palette")).toBeNull();
  });

  it("opens workspace variable management outside the API Client", () => {
    render(<DesktopApp />);

    fireEvent.click(screen.getByRole("button", { name: "Manage variables" }));

    expect(screen.getByText("Workspace manager: env-dev")).toBeTruthy();
    expect(screen.getByText("Workspace status: Default Workspace")).toBeTruthy();
    // Scheme A: keep the module activity bar while the manager replaces main.
    expect(screen.getByRole("button", { name: "Open command palette" })).toBeTruthy();
  });

  it("confirms before leaving dirty variable management via the activity bar", () => {
    setActiveTab.mockClear();
    render(<DesktopApp />);

    fireEvent.click(screen.getByRole("button", { name: "Manage variables" }));
    fireEvent.click(screen.getByRole("button", { name: "Mark variables dirty" }));
    fireEvent.click(screen.getByRole("button", { name: "Open SSH Terminal" }));

    expect(screen.getByRole("dialog", { name: "variables.discardChangesTitle" })).toBeTruthy();
    expect(screen.getByText("Workspace manager: env-dev")).toBeTruthy();
    expect(setActiveTab).not.toHaveBeenCalled();

    fireEvent.click(screen.getByRole("button", { name: "variables.discard" }));

    expect(screen.queryByText("Workspace manager: env-dev")).toBeNull();
    expect(setActiveTab).toHaveBeenCalledWith("ssh-main");
  });

  it("keeps dirty variable management open when leave is cancelled", () => {
    setActiveTab.mockClear();
    render(<DesktopApp />);

    fireEvent.click(screen.getByRole("button", { name: "Manage variables" }));
    fireEvent.click(screen.getByRole("button", { name: "Mark variables dirty" }));
    fireEvent.click(screen.getByRole("button", { name: "Open SSH Terminal" }));
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(screen.getByText("Workspace manager: env-dev")).toBeTruthy();
    expect(setActiveTab).not.toHaveBeenCalled();
  });
});
