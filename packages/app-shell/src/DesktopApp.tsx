import { ApiClientPage } from "@unfour/api-client";
import AppShell from "./AppShell";
import { DatabasePage } from "@unfour/database";
import { TerminalLogPanel, TerminalPage, TerminalStatusBar } from "@unfour/ssh-terminal";
import {
  WorkspaceEnvironmentsPage,
  WorkspaceEnvironmentsStatusBar,
} from "@unfour/workspace-environments";
import { useCallback, useMemo, useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  CommandPalette,
  ConfirmDialog,
  FeedbackProvider,
  MainWorkspace,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";
import {
  exportDiagnosticsBundle,
  getSystemHealth,
  getWorkspaceLayout,
  getWorkspaceState,
  listDatabaseConnections,
  listWorkspaceEnvironments,
  openDiagnosticsDir,
  openLogDir,
  setActiveWorkspace as setActiveWorkspaceCommand,
  setActiveWorkspaceEnvironment,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { AppTitleBar } from "./components/AppTitleBar";
import { BottomPanelPlaceholder } from "./components/BottomPanelPlaceholder";
import { LayoutControls } from "./components/LayoutControls";
import { ModuleActivityBar } from "./components/ModuleActivityBar";
import { ModuleSidebar } from "./components/ModuleSidebar";
import { RightInspectorPlaceholder } from "./components/RightInspectorPlaceholder";
import { StatusBarPlaceholder } from "./components/StatusBarPlaceholder";
import { CommandPaletteAction } from "./components/utils";
import { useLayoutPersistence } from "./components/useLayoutPersistence";
import { useWorkspaceInit } from "./components/useWorkspaceInit";
import type {
  DesktopAppExtensionContext,
  DesktopAppExtensions,
} from "./extensions";

export type DesktopAppProps = {
  extensions?: DesktopAppExtensions;
};

export function DesktopApp({ extensions }: DesktopAppProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
  const [bottomPanelCollapsed, setBottomPanelCollapsed] = useState(true);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [apiSidebarContent, setApiSidebarContent] = useState<ReactNode>(null);
  const [sshSidebarContent, setSshSidebarContent] = useState<ReactNode>(null);
  const [databaseSidebarContent, setDatabaseSidebarContent] = useState<ReactNode>(null);
  const [databaseStatusBarContent, setDatabaseStatusBarContent] = useState<ReactNode>(null);
  const [rightInspectorCollapsed, setRightInspectorCollapsed] = useState(true);
  const [variableManagerRequest, setVariableManagerRequest] = useState<{
    environmentId: string | null;
    nonce: number;
    workspaceId: string;
  } | null>(null);
  const [variableManagerDirty, setVariableManagerDirty] = useState(false);
  const [pendingVariableManagerLeave, setPendingVariableManagerLeave] = useState<
    | { kind: "activate-workspace"; workspaceId: string }
    | { kind: "select-module"; tabId: string }
    | { kind: "toggle-sidebar" }
    | null
  >(null);
  const {
    activeTabId,
    activeWorkspaceId,
    bottomPanelHeight,
    rightInspectorWidth,
    setActiveTab,
    setActiveWorkspace,
    setBottomPanelHeight,
    setRightInspectorWidth,
    setSelectedApiRequest,
    setSidebarWidth,
    sidebarCollapsed,
    sidebarWidth,
    toggleSidebar,
    tabs,
  } = useWorkspaceStore();
  const healthQuery = useQuery({ queryKey: ["system-health"], queryFn: getSystemHealth });
  const workspaceQuery = useQuery({ queryKey: ["workspaces"], queryFn: getWorkspaceState });
  const activeWorkspace =
    workspaceQuery.data?.workspaces.find(
      (w) => w.id === (activeWorkspaceId || workspaceQuery.data.activeWorkspaceId),
    ) ?? workspaceQuery.data?.workspaces[0];
  const workspaceLayoutQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-layout", activeWorkspace?.id],
    queryFn: () => getWorkspaceLayout(activeWorkspace?.id ?? ""),
  });
  const workspaceEnvironmentsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-environments", activeWorkspace?.id],
    queryFn: () => listWorkspaceEnvironments(activeWorkspace?.id ?? ""),
  });
  const sidebarDatabaseConnectionsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["database-connections", activeWorkspace?.id],
    queryFn: () => listDatabaseConnections(activeWorkspace?.id ?? ""),
  });
  useWorkspaceInit(workspaceQuery.data?.activeWorkspaceId, workspaceLayoutQuery.data, sidebarDatabaseConnectionsQuery.data);
  useLayoutPersistence(activeWorkspace?.id ?? null);
  const runCommandPaletteAction = useCallback(
    (action: () => void | Promise<unknown>) => {
      setCommandPaletteOpen(false);
      void Promise.resolve(action()).catch((error) =>
        handleError(error, { key: "feedback.command.actionFailed" }),
      );
    },
    [handleError],
  );
  const activateWorkspaceMutation = useMutation({
    mutationFn: setActiveWorkspaceCommand,
    onSuccess: (state) => {
      setVariableManagerRequest(null);
      setVariableManagerDirty(false);
      setPendingVariableManagerLeave(null);
      setActiveWorkspace(state.activeWorkspaceId);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });
  const activateEnvironmentMutation = useMutation({
    mutationFn: (input: { environmentId: string | null; workspaceId: string }) =>
      setActiveWorkspaceEnvironment(input.workspaceId, input.environmentId),
    onSuccess: (environments, input) => {
      queryClient.setQueryData(
        ["workspace-environments", input.workspaceId],
        environments,
      );
    },
    onError: (error) =>
      handleError(error, { key: "feedback.api.environmentActivateFailed" }),
  });
  const workspaceEnvironments = workspaceEnvironmentsQuery.data ?? [];
  const activeEnvironment =
    workspaceEnvironments.find((environment) => environment.isActive) ?? null;
  const variableManagerOpen =
    Boolean(activeWorkspace) &&
    variableManagerRequest?.workspaceId === activeWorkspace?.id;
  const handleManageVariables = useCallback(() => {
    if (!activeWorkspace) return;
    setVariableManagerRequest((current) => ({
      environmentId: activeEnvironment?.id ?? null,
      nonce: (current?.nonce ?? 0) + 1,
      workspaceId: activeWorkspace.id,
    }));
  }, [activeEnvironment?.id, activeWorkspace]);
  const closeVariableManager = useCallback(() => {
    setVariableManagerRequest(null);
    setVariableManagerDirty(false);
    setPendingVariableManagerLeave(null);
  }, []);
  const applyVariableManagerLeave = useCallback(
    (
      leave:
        | { kind: "activate-workspace"; workspaceId: string }
        | { kind: "select-module"; tabId: string }
        | { kind: "toggle-sidebar" },
    ) => {
      closeVariableManager();
      if (leave.kind === "select-module") {
        setActiveTab(leave.tabId);
        return;
      }
      if (leave.kind === "activate-workspace") {
        activateWorkspaceMutation.mutate(leave.workspaceId);
      }
    },
    [activateWorkspaceMutation, closeVariableManager, setActiveTab],
  );
  const requestLeaveVariableManager = useCallback(
    (
      leave:
        | { kind: "activate-workspace"; workspaceId: string }
        | { kind: "select-module"; tabId: string }
        | { kind: "toggle-sidebar" },
    ) => {
      if (!variableManagerOpen) {
        if (leave.kind === "select-module") {
          setActiveTab(leave.tabId);
          return;
        }
        if (leave.kind === "toggle-sidebar") {
          toggleSidebar();
          return;
        }
        activateWorkspaceMutation.mutate(leave.workspaceId);
        return;
      }
      if (variableManagerDirty) {
        setPendingVariableManagerLeave(leave);
        return;
      }
      applyVariableManagerLeave(leave);
    },
    [
      activateWorkspaceMutation,
      applyVariableManagerLeave,
      setActiveTab,
      toggleSidebar,
      variableManagerDirty,
      variableManagerOpen,
    ],
  );
  const handleSelectModule = useCallback(
    (tabId: string) => {
      requestLeaveVariableManager({ kind: "select-module", tabId });
    },
    [requestLeaveVariableManager],
  );
  const handleToggleSidebar = useCallback(() => {
    if (variableManagerOpen) {
      requestLeaveVariableManager({ kind: "toggle-sidebar" });
      return;
    }
    toggleSidebar();
  }, [requestLeaveVariableManager, toggleSidebar, variableManagerOpen]);
  const handleActivateWorkspace = useCallback(
    (workspaceId: string) => {
      requestLeaveVariableManager({ kind: "activate-workspace", workspaceId });
    },
    [requestLeaveVariableManager],
  );
  const handleApiSidebarChange = useCallback((content: ReactNode | null) => {
    setApiSidebarContent(content);
  }, []);
  const handleSshSidebarChange = useCallback((content: ReactNode | null) => {
    setSshSidebarContent(content);
  }, []);
  const handleDatabaseSidebarChange = useCallback((content: ReactNode | null) => {
    setDatabaseSidebarContent(content);
  }, []);
  const handleDatabaseStatusBarChange = useCallback((content: ReactNode | null) => {
    setDatabaseStatusBarContent(content);
  }, []);
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];
  const extensionContext: DesktopAppExtensionContext = useMemo(
    () => ({ activeTab, activeWorkspace }),
    [activeTab, activeWorkspace],
  );
  const TitleBarEnd = extensions?.titleBarEnd;
  const StatusBarEnd = extensions?.statusBarEnd;
  const Overlays = extensions?.overlays;
  const layoutControls = useMemo(
    () => (
      <LayoutControls
        bottomPanelCollapsed={bottomPanelCollapsed}
        onToggleBottomPanel={() => setBottomPanelCollapsed((collapsed) => !collapsed)}
        onToggleInspector={() => setRightInspectorCollapsed((collapsed) => !collapsed)}
        onToggleSidebar={toggleSidebar}
        rightInspectorCollapsed={rightInspectorCollapsed}
        sidebarCollapsed={sidebarCollapsed}
      />
    ),
    [
      bottomPanelCollapsed,
      rightInspectorCollapsed,
      sidebarCollapsed,
      toggleSidebar,
    ],
  );
  const statusBarRightAccessory = useMemo(
    () => (
      <>
        {layoutControls}
        {StatusBarEnd && <StatusBarEnd {...extensionContext} />}
      </>
    ),
    [extensionContext, layoutControls, StatusBarEnd],
  );
  return (
    <FeedbackProvider>
      <AppShell
        activityBar={
          <ModuleActivityBar
            activeKind={activeTab.kind}
            onOpenCommandPalette={() => setCommandPaletteOpen(true)}
            sidebarCollapsed={sidebarCollapsed || variableManagerOpen}
            onSelect={handleSelectModule}
            onToggleSidebar={handleToggleSidebar}
          />
        }
        bottomPanel={
          variableManagerOpen ? undefined : activeTab.kind === "ssh" && activeWorkspace ? (
            <TerminalLogPanel
              collapsed={bottomPanelCollapsed}
              height={bottomPanelHeight}
              onCollapse={() => setBottomPanelCollapsed(true)}
              onHeightChange={setBottomPanelHeight}
              workspaceId={activeWorkspace.id}
            />
          ) : (
            <BottomPanelPlaceholder
              collapsed={bottomPanelCollapsed}
              height={bottomPanelHeight}
              onCollapse={() => setBottomPanelCollapsed(true)}
              onHeightChange={setBottomPanelHeight}
            />
          )
        }
        globalToolbar={
          <AppTitleBar
            activeEnvironmentId={activeEnvironment?.id ?? null}
            activeWorkspace={activeWorkspace}
            environments={workspaceEnvironments}
            endAccessory={TitleBarEnd ? <TitleBarEnd {...extensionContext} /> : undefined}
            extensionContext={extensionContext}
            onActivateWorkspace={handleActivateWorkspace}
            onManageVariables={handleManageVariables}
            onSelectEnvironment={(environmentId) =>
              activeWorkspace &&
              activateEnvironmentMutation.mutate({
                environmentId,
                workspaceId: activeWorkspace.id,
              })
            }
            settingsSections={extensions?.settingsSections}
            workspaces={workspaceQuery.data?.workspaces ?? []}
          />
        }
        rightInspector={
          variableManagerOpen ? undefined : (
            <RightInspectorPlaceholder
              activeTab={activeTab}
              collapsed={rightInspectorCollapsed}
              onCollapse={() => setRightInspectorCollapsed(true)}
              onWidthChange={setRightInspectorWidth}
              width={rightInspectorWidth}
            />
          )
        }
        sidebar={
          <ModuleSidebar
            activeTab={activeTab}
            apiSidebarContent={apiSidebarContent}
            collapsed={sidebarCollapsed || variableManagerOpen}
            databaseSidebarContent={databaseSidebarContent}
            onWidthChange={setSidebarWidth}
            sshSidebarContent={sshSidebarContent}
            width={sidebarWidth}
          />
        }
        statusBar={
          variableManagerOpen && activeWorkspace ? (
            <WorkspaceEnvironmentsStatusBar workspaceName={activeWorkspace.name} />
          ) : activeTab.kind === "ssh" && activeWorkspace ? (
            <TerminalStatusBar
              rightAccessory={statusBarRightAccessory}
              workspaceId={activeWorkspace.id}
              workspaceName={activeWorkspace.name}
            />
          ) : activeTab.kind === "database" && databaseStatusBarContent ? (
            databaseStatusBarContent
          ) : (
            <StatusBarPlaceholder
              activeTab={activeTab}
              activeWorkspace={activeWorkspace}
              healthReady={healthQuery.data?.storageReady === true}
              rightAccessory={statusBarRightAccessory}
              syncStrategy={healthQuery.data?.syncStrategy ?? "local-first"}
            />
          )
        }
        main={
          <MainWorkspace
            className="[&>section]:p-0"
            tabBar={null}
          >
            {activeWorkspace && (
              <div
                className={
                  activeTab.kind === "api" && !variableManagerOpen ? "h-full" : "hidden"
                }
              >
                <ApiClientPage
                  active={activeTab.kind === "api" && !variableManagerOpen}
                  onShellSidebarChange={handleApiSidebarChange}
                  onActiveSavedRequestChange={setSelectedApiRequest}
                  openIntent={null}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {activeTab.kind === "ssh" && activeWorkspace && (
              <div className={variableManagerOpen ? "hidden" : "h-full"}>
                <TerminalPage
                  onShellSidebarChange={handleSshSidebarChange}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {/* Keep DatabasePage mounted across module switches (mirrors the
                ApiClientPage pattern above). The SQL editor is a Monaco instance
                that remounts from scratch when this subtree is conditionally
                unmounted, which repaints the editor with Monaco's default white
                `vs` theme for one frame before `handleMount` applies the
                unfour theme — the white flash seen when entering the database
                module with a query tab open. Mounting it always (hidden when
                inactive) preserves the live editor instance and its theme. */}
            {activeWorkspace && (
              <div
                className={
                  activeTab.kind === "database" && !variableManagerOpen
                    ? "h-full"
                    : "hidden"
                }
              >
                <DatabasePage
                  onShellSidebarChange={handleDatabaseSidebarChange}
                  onShellStatusBarChange={handleDatabaseStatusBarChange}
                  statusBarRightAccessory={statusBarRightAccessory}
                  workspaceName={activeWorkspace.name}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {activeWorkspace && variableManagerOpen && variableManagerRequest && (
              <WorkspaceEnvironmentsPage
                initialEnvironmentId={variableManagerRequest.environmentId}
                key={`${activeWorkspace.id}:${variableManagerRequest.nonce}`}
                onClose={closeVariableManager}
                onDirtyChange={setVariableManagerDirty}
                workspaceId={activeWorkspace.id}
              />
            )}
          </MainWorkspace>
        }
      />
      <ConfirmDialog
        confirmLabel={t("variables.discard")}
        description={t("variables.discardChangesDescription")}
        onConfirm={() => {
          if (!pendingVariableManagerLeave) return;
          applyVariableManagerLeave(pendingVariableManagerLeave);
        }}
        onOpenChange={(open) => !open && setPendingVariableManagerLeave(null)}
        open={pendingVariableManagerLeave !== null}
        title={t("variables.discardChangesTitle")}
      />
      <CommandPalette
        actions={
          <>
            <CommandPaletteAction
              onSelect={() => runCommandPaletteAction(() => handleSelectModule("api-main"))}
            >
              {t("app.commandPalette.openApiClient")}
            </CommandPaletteAction>
            <CommandPaletteAction
              onSelect={() =>
                runCommandPaletteAction(() => handleSelectModule("database-main"))
              }
            >
              {t("app.commandPalette.openDatabase")}
            </CommandPaletteAction>
            <CommandPaletteAction
              onSelect={() => runCommandPaletteAction(() => handleSelectModule("ssh-main"))}
            >
              {t("app.commandPalette.openSshTerminal")}
            </CommandPaletteAction>
            <CommandPaletteAction onSelect={() => runCommandPaletteAction(openLogDir)}>
              {t("app.commandPalette.openLogDir")}
            </CommandPaletteAction>
            <CommandPaletteAction onSelect={() => runCommandPaletteAction(openDiagnosticsDir)}>
              {t("app.commandPalette.openDiagnosticsDir")}
            </CommandPaletteAction>
            <CommandPaletteAction onSelect={() => runCommandPaletteAction(exportDiagnosticsBundle)}>
              {t("app.commandPalette.exportDiagnosticsBundle")}
            </CommandPaletteAction>
            {extensions?.commandPaletteActions?.map((action) => (
              <CommandPaletteAction
                key={action.id}
                onSelect={() => runCommandPaletteAction(() => action.run(extensionContext))}
              >
                {action.label}
              </CommandPaletteAction>
            ))}
          </>
        }
        onClose={() => setCommandPaletteOpen(false)}
        open={commandPaletteOpen}
      />
      {Overlays && <Overlays {...extensionContext} />}
    </FeedbackProvider>
  );
}

export default DesktopApp;
