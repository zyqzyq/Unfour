import AppShell from "./AppShell";
import { useCallback, useMemo, useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Button,
  EmptyState,
  ErrorState,
  LoadingState,
  ConfirmDialog,
  FeedbackProvider,
  MainWorkspace,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";
import {
  getSystemHealth,
  getWorkspaceLayout,
  getWorkspaceState,
  listDatabaseConnections,
  listWorkspaceEnvironments,
  setActiveWorkspace as setActiveWorkspaceCommand,
  setActiveWorkspaceEnvironment,
  type WorkspaceState,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { AppTitleBar } from "./components/AppTitleBar";
import { DesktopCommandPalette } from "./components/DesktopCommandPalette";
import {
  ApiClientModule, DatabaseModule, SshTerminalLogPanel, SshTerminalModule,
  SshTerminalStatusBar, WorkspaceEnvironmentsModule,
  WorkspaceEnvironmentsModuleStatusBar,
} from "./components/LazyFeatureModules";
import { LayoutControls } from "./components/LayoutControls";
import { ModuleActivityBar } from "./components/ModuleActivityBar";
import { ModuleSidebar } from "./components/ModuleSidebar";
import { StatusBarPlaceholder } from "./components/StatusBarPlaceholder";
import { useLayoutPersistence } from "./components/useLayoutPersistence";
import { useFeatureModulePreload } from "./components/useFeatureModulePreload";
import { usePersistentFeatureMounts } from "./components/usePersistentFeatureMounts";
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
  const [variableManagerRequest, setVariableManagerRequest] = useState<{
    environmentId: string | null;
    workspaceId: string;
  } | null>(null);
  const [variableManagerDirty, setVariableManagerDirty] = useState(false);
  const [pendingVariableManagerLeave, setPendingVariableManagerLeave] = useState<
    | { kind: "activate-workspace"; workspaceId: string }
    | { kind: "select-module"; tabId: string }
    | null
  >(null);
  const {
    activeTabId,
    activeWorkspaceId,
    bottomPanelHeight,
    setActiveTab: setActiveTabInStore,
    setActiveWorkspace,
    setBottomPanelHeight,
    setSelectedApiRequest,
    setModuleSidebarWidth,
    sidebarCollapsed,
    sidebarWidths,
    toggleSidebar,
    tabs,
  } = useWorkspaceStore();
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];
  const { setActiveTab, shouldMountApi, shouldMountDatabase, shouldMountSsh } =
    usePersistentFeatureMounts({
      activeTabId,
      setActiveTab: setActiveTabInStore,
      tabs,
    });
  const healthQuery = useQuery({ queryKey: ["system-health"], queryFn: getSystemHealth });
  const workspaceQuery = useQuery({ queryKey: ["workspaces"], queryFn: getWorkspaceState });
  const activeWorkspace =
    workspaceQuery.data?.workspaces.find(
      (w) => w.id === (activeWorkspaceId || workspaceQuery.data.activeWorkspaceId),
    ) ?? workspaceQuery.data?.workspaces[0];
  const handlePreloadFeature = useFeatureModulePreload(activeTab.kind, {
    queryClient,
    workspaceId: activeWorkspace?.id,
  });
  const workspaceLayoutQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-layout", activeWorkspace?.id],
    queryFn: () => getWorkspaceLayout(activeWorkspace?.id ?? ""),
  });
  const workspaceEnvironmentsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["workspace-environments", activeWorkspace?.id],
    queryFn: () => listWorkspaceEnvironments(activeWorkspace?.id ?? ""),
    refetchOnWindowFocus: true,
  });
  const sidebarDatabaseConnectionsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["database-connections", activeWorkspace?.id],
    queryFn: () => listDatabaseConnections(activeWorkspace?.id ?? ""),
  });
  useWorkspaceInit(workspaceQuery.data?.activeWorkspaceId, workspaceLayoutQuery.data, sidebarDatabaseConnectionsQuery.data);
  useLayoutPersistence(activeWorkspace?.id ?? null);
  const activateWorkspaceMutation = useMutation({
    mutationFn: setActiveWorkspaceCommand,
    onMutate: async (workspaceId) => {
      await queryClient.cancelQueries({ queryKey: ["workspaces"] });
      const previousState = queryClient.getQueryData<WorkspaceState>(["workspaces"]);
      const previousWorkspaceId = activeWorkspaceId;
      if (previousState) {
        queryClient.setQueryData<WorkspaceState>(["workspaces"], {
          ...previousState,
          activeWorkspaceId: workspaceId,
        });
      }
      setActiveWorkspace(workspaceId);
      return { previousState, previousWorkspaceId };
    },
    onSuccess: (state) => {
      setVariableManagerRequest(null);
      setVariableManagerDirty(false);
      setPendingVariableManagerLeave(null);
      queryClient.setQueryData<WorkspaceState>(["workspaces"], state);
      setActiveWorkspace(state.activeWorkspaceId);
    },
    onError: (error, _workspaceId, context) => {
      if (context?.previousState) {
        queryClient.setQueryData(["workspaces"], context.previousState);
      }
      const rollbackWorkspaceId =
        context?.previousWorkspaceId ?? context?.previousState?.activeWorkspaceId;
      if (rollbackWorkspaceId) {
        setActiveWorkspace(rollbackWorkspaceId);
      }
      handleError(error, { key: "feedback.workspace.activateFailed" });
    },
    onSettled: () => queryClient.invalidateQueries({ queryKey: ["workspaces"] }),
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
    if (!activeWorkspace || variableManagerOpen) return;
    setVariableManagerRequest({
      environmentId: activeEnvironment?.id ?? null,
      workspaceId: activeWorkspace.id,
    });
  }, [activeEnvironment?.id, activeWorkspace, variableManagerOpen]);
  const refreshWorkspaceEnvironments = useCallback(() => {
    if (!activeWorkspace?.id) return;
    void queryClient.refetchQueries({
      queryKey: ["workspace-environments", activeWorkspace.id],
    });
  }, [activeWorkspace, queryClient]);
  const closeVariableManager = useCallback(() => {
    setVariableManagerRequest(null);
    setVariableManagerDirty(false);
    setPendingVariableManagerLeave(null);
  }, []);
  const applyVariableManagerLeave = useCallback(
    (
      leave:
        | { kind: "activate-workspace"; workspaceId: string }
        | { kind: "select-module"; tabId: string },
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
        | { kind: "select-module"; tabId: string },
    ) => {
      if (!variableManagerOpen) {
        if (leave.kind === "select-module") {
          setActiveTab(leave.tabId);
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
      variableManagerDirty,
      variableManagerOpen,
    ],
  );
  const handleSelectModule = useCallback(
    (tabId: string) => {
      const kind = tabs.find((tab) => tab.id === tabId)?.kind;
      if (kind) {
        void handlePreloadFeature(kind).catch(() => undefined);
      }
      requestLeaveVariableManager({ kind: "select-module", tabId });
    },
    [handlePreloadFeature, requestLeaveVariableManager, tabs],
  );
  const handleActivateWorkspace = useCallback(
    (workspaceId: string) => {
      if (workspaceId === activeWorkspace?.id || activateWorkspaceMutation.isPending) return;
      requestLeaveVariableManager({ kind: "activate-workspace", workspaceId });
    },
    [activateWorkspaceMutation.isPending, activeWorkspace?.id, requestLeaveVariableManager],
  );
  const refreshWorkspaces = useCallback(
    async () => {
      await queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
    [queryClient],
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
  const extensionContext: DesktopAppExtensionContext = useMemo(
    () => ({
      activeTab,
      activeWorkspace,
      activateWorkspace: handleActivateWorkspace,
      refreshWorkspaces,
    }),
    [activeTab, activeWorkspace, handleActivateWorkspace, refreshWorkspaces],
  );
  const TitleBarEnd = extensions?.titleBarEnd;
  const StatusBarEnd = extensions?.statusBarEnd;
  const Overlays = extensions?.overlays;
  const WorkspaceVariableDecoration = extensions?.workspaceVariableDecoration;
  const layoutControls = useMemo(
    () => (
      !variableManagerOpen && <LayoutControls
        bottomPanelCollapsed={bottomPanelCollapsed}
        onToggleBottomPanel={activeTab.kind === "ssh"
          ? () => setBottomPanelCollapsed((collapsed) => !collapsed) : undefined}
        onToggleSidebar={toggleSidebar}
        sidebarCollapsed={sidebarCollapsed}
      />
    ),
    [
      bottomPanelCollapsed,
      activeTab.kind,
      variableManagerOpen,
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
            activeKind={variableManagerOpen ? null : activeTab.kind}
            onOpenCommandPalette={() => setCommandPaletteOpen(true)}
            onPreload={handlePreloadFeature}
            sidebarCollapsed={sidebarCollapsed || variableManagerOpen}
            onSelect={handleSelectModule}
            onToggleSidebar={toggleSidebar}
          />
        }
        bottomPanel={
          !variableManagerOpen && activeTab.kind === "ssh" && activeWorkspace ? (
            <SshTerminalLogPanel
              fallback={null}
              collapsed={bottomPanelCollapsed}
              height={bottomPanelHeight}
              onCollapse={() => setBottomPanelCollapsed(true)}
              onHeightChange={setBottomPanelHeight}
              workspaceId={activeWorkspace.id}
            />
          ) : undefined
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
            onOpenEnvironmentMenu={refreshWorkspaceEnvironments}
            onSelectEnvironment={(environmentId) =>
              activeWorkspace &&
              activateEnvironmentMutation.mutate({
                environmentId,
                workspaceId: activeWorkspace.id,
              })
            }
            settingsSections={extensions?.settingsSections}
            workspaceActions={extensions?.workspaceActions}
            workspaceDecoration={extensions?.workspaceDecoration}
            workspaceMenuActions={extensions?.workspaceMenuActions}
            workspaceMenuFooterActions={extensions?.workspaceMenuFooterActions}
            workspaces={workspaceQuery.data?.workspaces ?? []}
          />
        }
        sidebar={
          <ModuleSidebar
            activeTab={activeTab}
            apiSidebarContent={apiSidebarContent}
            collapsed={sidebarCollapsed || variableManagerOpen}
            databaseSidebarContent={databaseSidebarContent}
            onModuleWidthChange={setModuleSidebarWidth}
            sidebarWidths={sidebarWidths}
            sshSidebarContent={sshSidebarContent}
          />
        }
        statusBar={
          variableManagerOpen && activeWorkspace ? (
            <WorkspaceEnvironmentsModuleStatusBar
              fallback={
                <StatusBarPlaceholder
                  activeTab={activeTab}
                  activeWorkspace={activeWorkspace}
                  healthReady={healthQuery.data?.storageReady}
                  healthError={healthQuery.isError}
                  rightAccessory={statusBarRightAccessory}
                />
              }
              workspaceName={activeWorkspace.name}
            />
          ) : activeTab.kind === "ssh" && activeWorkspace ? (
            <SshTerminalStatusBar
              fallback={
                <StatusBarPlaceholder
                  activeTab={activeTab}
                  activeWorkspace={activeWorkspace}
                  healthReady={healthQuery.data?.storageReady}
                  healthError={healthQuery.isError}
                  rightAccessory={statusBarRightAccessory}
                />
              }
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
              healthReady={healthQuery.data?.storageReady}
              healthError={healthQuery.isError}
              rightAccessory={statusBarRightAccessory}
            />
          )
        }
        main={
          <MainWorkspace
            className="[&>section]:p-0"
            tabBar={null}
          >
            {!activeWorkspace && (workspaceQuery.isError ? (
              <ErrorState className="h-full rounded-none border-0">
                <div className="space-y-2" role="alert">
                  <p>{t("app.workspace.loadFailed")}</p>
                  <Button disabled={workspaceQuery.isFetching} onClick={() => void workspaceQuery.refetch()} size="sm">
                    {t("common.actions.retry")}
                  </Button>
                </div>
              </ErrorState>
            ) : workspaceQuery.isPending ? (
              <LoadingState className="h-full rounded-none border-0" />
            ) : (
              <EmptyState className="h-full rounded-none border-0">
                {t("app.workspace.createToStart")}
              </EmptyState>
            ))}
            {activeWorkspace && shouldMountApi && (
              <div
                className={
                  activeTab.kind === "api" && !variableManagerOpen ? "h-full" : "hidden"
                }
              >
                <ApiClientModule
                  active={activeTab.kind === "api" && !variableManagerOpen}
                  onShellSidebarChange={handleApiSidebarChange}
                  onActiveSavedRequestChange={setSelectedApiRequest}
                  openIntent={null}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {activeWorkspace && shouldMountSsh && (
              <div
                className={
                  activeTab.kind === "ssh" && !variableManagerOpen ? "h-full" : "hidden"
                }
              >
                <SshTerminalModule
                  active={activeTab.kind === "ssh" && !variableManagerOpen}
                  onShellSidebarChange={handleSshSidebarChange}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {/* Keep DatabasePage mounted after its first visit. Reusing the
                Monaco instance preserves its theme and avoids a white repaint
                when the user returns to a query tab. */}
            {activeWorkspace && shouldMountDatabase && (
              <div
                className={
                  activeTab.kind === "database" && !variableManagerOpen
                    ? "h-full"
                    : "hidden"
                }
              >
                <DatabaseModule
                  active={activeTab.kind === "database" && !variableManagerOpen}
                  onShellSidebarChange={handleDatabaseSidebarChange}
                  onShellStatusBarChange={handleDatabaseStatusBarChange}
                  statusBarRightAccessory={statusBarRightAccessory}
                  workspaceName={activeWorkspace.name}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {activeWorkspace && variableManagerOpen && variableManagerRequest && (
              <WorkspaceEnvironmentsModule
                initialEnvironmentId={variableManagerRequest.environmentId}
                key={activeWorkspace.id}
                onClose={closeVariableManager}
                onDirtyChange={setVariableManagerDirty}
                variableDecoration={
                  WorkspaceVariableDecoration
                    ? (variable) => (
                        <WorkspaceVariableDecoration
                          {...extensionContext}
                          variable={variable}
                        />
                      )
                    : undefined
                }
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
      <DesktopCommandPalette
        extensionContext={extensionContext}
        extensionActions={extensions?.commandPaletteActions}
        onClose={() => setCommandPaletteOpen(false)}
        onOpen={() => setCommandPaletteOpen(true)}
        onSelectModule={handleSelectModule}
        onManageVariables={activeWorkspace ? handleManageVariables : undefined}
        open={commandPaletteOpen}
      />
      {Overlays && <Overlays {...extensionContext} />}
    </FeedbackProvider>
  );
}

export default DesktopApp;
