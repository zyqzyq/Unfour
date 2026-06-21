import {
  ApiDebuggerPage,
} from "@unfour/api-client";
import AppShell from "./AppShell";
import { DatabasePage } from "@unfour/database";
import { TerminalLogPanel, TerminalPage, TerminalStatusBar } from "@unfour/ssh-terminal";
import { useCallback, useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CommandPalette, MainWorkspace, useI18n } from "@unfour/ui";
import {
  getSystemHealth,
  getWorkspaceLayout,
  getWorkspaceState,
  listDatabaseConnections,
  setActiveWorkspace as setActiveWorkspaceCommand,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { AppTitleBar } from "./components/AppTitleBar";
import { BottomPanelPlaceholder } from "./components/BottomPanelPlaceholder";
import { ModuleActivityBar } from "./components/ModuleActivityBar";
import { ModuleSidebar } from "./components/ModuleSidebar";
import { RightInspectorPlaceholder } from "./components/RightInspectorPlaceholder";
import { StatusBarPlaceholder } from "./components/StatusBarPlaceholder";
import { CommandPaletteAction } from "./components/utils";
import { useLayoutPersistence } from "./components/useLayoutPersistence";
import { useWorkspaceInit } from "./components/useWorkspaceInit";

export function DesktopApp() {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [bottomPanelCollapsed, setBottomPanelCollapsed] = useState(true);
  const [bottomPanelHeight, setBottomPanelHeight] = useState(220);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [apiSidebarContent, setApiSidebarContent] = useState<ReactNode>(null);
  const [sshSidebarContent, setSshSidebarContent] = useState<ReactNode>(null);
  const [rightInspectorCollapsed, setRightInspectorCollapsed] = useState(true);
  const [rightInspectorWidth, setRightInspectorWidth] = useState(300);
  const [sidebarWidth, setSidebarWidth] = useState(248);
  const {
    activeTabId,
    activeWorkspaceId,
    selectedDatabaseConnectionId,
    setActiveTab,
    setActiveWorkspace,
    setSelectedApiRequest,
    sidebarCollapsed,
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
  const sidebarDatabaseConnectionsQuery = useQuery({
    enabled: Boolean(activeWorkspace?.id),
    queryKey: ["database-connections", activeWorkspace?.id],
    queryFn: () => listDatabaseConnections(activeWorkspace?.id ?? ""),
  });
  useWorkspaceInit(workspaceQuery.data?.activeWorkspaceId, workspaceLayoutQuery.data, sidebarDatabaseConnectionsQuery.data);
  useLayoutPersistence(activeWorkspace?.id ?? null);
  const activateWorkspaceMutation = useMutation({
    mutationFn: setActiveWorkspaceCommand,
    onSuccess: (state) => {
      setActiveWorkspace(state.activeWorkspaceId);
      queryClient.invalidateQueries({ queryKey: ["workspaces"] });
    },
  });
  const handleApiSidebarChange = useCallback((content: ReactNode | null) => {
    setApiSidebarContent(content);
  }, []);
  const handleSshSidebarChange = useCallback((content: ReactNode | null) => {
    setSshSidebarContent(content);
  }, []);
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];
  return (
    <>
      <AppShell
        activityBar={
          <ModuleActivityBar
            activeKind={activeTab.kind}
            onOpenCommandPalette={() => setCommandPaletteOpen(true)}
            sidebarCollapsed={sidebarCollapsed}
            onSelect={setActiveTab}
            onToggleSidebar={toggleSidebar}
          />
        }
        bottomPanel={
          activeTab.kind === "ssh" && activeWorkspace ? (
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
            activeWorkspace={activeWorkspace}
            bottomPanelCollapsed={bottomPanelCollapsed}
            healthReady={healthQuery.data?.storageReady === true}
            onActivateWorkspace={(id) => activateWorkspaceMutation.mutate(id)}
            onOpenCommandPalette={() => setCommandPaletteOpen(true)}
            onToggleBottomPanel={() => setBottomPanelCollapsed((c) => !c)}
            onToggleInspector={() => setRightInspectorCollapsed((c) => !c)}
            onToggleSidebar={toggleSidebar}
            rightInspectorCollapsed={rightInspectorCollapsed}
            sidebarCollapsed={sidebarCollapsed}
            syncStrategy={healthQuery.data?.syncStrategy ?? "local-first"}
            workspaces={workspaceQuery.data?.workspaces ?? []}
          />
        }
        rightInspector={
          <RightInspectorPlaceholder
            activeTab={activeTab}
            collapsed={rightInspectorCollapsed}
            onCollapse={() => setRightInspectorCollapsed(true)}
            onWidthChange={setRightInspectorWidth}
            width={rightInspectorWidth}
          />
        }
        sidebar={
          <ModuleSidebar
            activeTab={activeTab}
            activeTabId={activeTabId}
            apiSidebarContent={apiSidebarContent}
            collapsed={sidebarCollapsed}
            onWidthChange={setSidebarWidth}
            selectedDatabaseConnectionId={selectedDatabaseConnectionId}
            setActiveTab={setActiveTab}
            sshSidebarContent={sshSidebarContent}
            width={sidebarWidth}
          />
        }
        statusBar={
          activeTab.kind === "ssh" && activeWorkspace ? (
            <TerminalStatusBar
              workspaceId={activeWorkspace.id}
              workspaceName={activeWorkspace.name}
            />
          ) : (
            <StatusBarPlaceholder
              activeTab={activeTab}
              activeWorkspace={activeWorkspace}
              healthReady={healthQuery.data?.storageReady === true}
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
              <div className={activeTab.kind === "api" ? "h-full" : "hidden"}>
                <ApiDebuggerPage
                  key={activeWorkspace.id}
                  onShellSidebarChange={handleApiSidebarChange}
                  onActiveSavedRequestChange={setSelectedApiRequest}
                  openIntent={null}
                  workspaceId={activeWorkspace.id}
                />
              </div>
            )}
            {activeTab.kind === "ssh" && activeWorkspace && (
              <TerminalPage
                onShellSidebarChange={handleSshSidebarChange}
                workspaceId={activeWorkspace.id}
              />
            )}
            {activeTab.kind === "database" && activeWorkspace && (
              <DatabasePage workspaceId={activeWorkspace.id} />
            )}
          </MainWorkspace>
        }
      />
      <CommandPalette
        actions={
          <>
            <CommandPaletteAction onSelect={() => setActiveTab("api-main")}>
              {t("app.commandPalette.openApiClient")}
            </CommandPaletteAction>
            <CommandPaletteAction onSelect={() => setActiveTab("database-main")}>
              {t("app.commandPalette.openDatabase")}
            </CommandPaletteAction>
            <CommandPaletteAction onSelect={() => setActiveTab("ssh-main")}>
              {t("app.commandPalette.openSshTerminal")}
            </CommandPaletteAction>
          </>
        }
        onClose={() => setCommandPaletteOpen(false)}
        open={commandPaletteOpen}
      />
    </>
  );
}

export default DesktopApp;
