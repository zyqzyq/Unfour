import AppShell from "./AppShell";
import { useMemo, useState, type ReactNode } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { FeedbackProvider } from "@unfour/ui";
import { useWorkspaceStore } from "@unfour/workspace-core";
import { AppTitleBar } from "./components/AppTitleBar";
import { DesktopOverlays } from "./components/DesktopOverlays";
import { DesktopStatusBar } from "./components/DesktopStatusBar";
import { DesktopWorkspaceContent } from "./components/DesktopWorkspaceContent";
import { SshTerminalLogPanel } from "./components/LazyFeatureModules";
import { LayoutControls } from "./components/LayoutControls";
import { ModuleActivityBar } from "./components/ModuleActivityBar";
import { ModuleSidebar } from "./components/ModuleSidebar";
import { useDesktopNavigation } from "./components/useDesktopNavigation";
import { useDesktopWorkspace } from "./components/useDesktopWorkspace";
import { useFeatureModulePreload } from "./components/useFeatureModulePreload";
import { usePersistentFeatureMounts } from "./components/usePersistentFeatureMounts";
import type { DesktopAppExtensionContext, DesktopAppExtensions } from "./extensions";

export type DesktopAppProps = { extensions?: DesktopAppExtensions };

export function DesktopApp({ extensions }: DesktopAppProps) {
  const queryClient = useQueryClient();
  const [bottomPanelCollapsed, setBottomPanelCollapsed] = useState(true);
  const [commandPaletteOpen, setCommandPaletteOpen] = useState(false);
  const [apiSidebarContent, setApiSidebarContent] = useState<ReactNode>(null);
  const [flowSidebarContent, setFlowSidebarContent] = useState<ReactNode>(null);
  const [sshSidebarContent, setSshSidebarContent] = useState<ReactNode>(null);
  const [databaseSidebarContent, setDatabaseSidebarContent] = useState<ReactNode>(null);
  const [databaseStatusBarContent, setDatabaseStatusBarContent] = useState<ReactNode>(null);
  const {
    activeTabId, bottomPanelHeight, setActiveTab: setActiveTabInStore,
    setBottomPanelHeight, setSelectedApiRequest, setModuleSidebarWidth,
    sidebarCollapsed, sidebarWidths, toggleSidebar, tabs,
  } = useWorkspaceStore();
  const activeTab = tabs.find((tab) => tab.id === activeTabId) ?? tabs[0];
  const mounts = usePersistentFeatureMounts({ activeTabId, setActiveTab: setActiveTabInStore, tabs });
  const workspace = useDesktopWorkspace();
  const { activeWorkspace, activeEnvironment, workspaceEnvironments, workspaceQuery, refreshWorkspaces } = workspace;
  const handlePreloadFeature = useFeatureModulePreload(activeTab.kind, { queryClient, workspaceId: activeWorkspace?.id });
  const navigation = useDesktopNavigation({
    activeWorkspace, activeEnvironmentId: activeEnvironment?.id ?? null,
    tabs, setActiveTab: mounts.setActiveTab, preloadFeature: handlePreloadFeature,
  });
  const { variableManagerOpen, handleActivateWorkspace, handleManageVariables, handleSelectModule } = navigation;
  const extensionContext: DesktopAppExtensionContext = useMemo(
    () => ({ activeTab, activeWorkspace, activateWorkspace: handleActivateWorkspace, refreshWorkspaces }),
    [activeTab, activeWorkspace, handleActivateWorkspace, refreshWorkspaces],
  );
  const TitleBarEnd = extensions?.titleBarEnd;
  const StatusBarEnd = extensions?.statusBarEnd;
  const layoutControls = useMemo(
    () => !variableManagerOpen && <LayoutControls
      bottomPanelCollapsed={bottomPanelCollapsed}
      onToggleBottomPanel={activeTab.kind === "ssh" ? () => setBottomPanelCollapsed((collapsed) => !collapsed) : undefined}
      onToggleSidebar={toggleSidebar}
      sidebarCollapsed={sidebarCollapsed}
    />,
    [bottomPanelCollapsed, activeTab.kind, variableManagerOpen, sidebarCollapsed, toggleSidebar],
  );
  const statusBarRightAccessory = useMemo(
    () => <>{layoutControls}{StatusBarEnd && <StatusBarEnd {...extensionContext} />}</>,
    [extensionContext, layoutControls, StatusBarEnd],
  );
  return <FeedbackProvider>
    <AppShell
      activityBar={<ModuleActivityBar
        activeKind={variableManagerOpen ? null : activeTab.kind}
        onOpenCommandPalette={() => setCommandPaletteOpen(true)}
        onPreload={handlePreloadFeature}
        sidebarCollapsed={sidebarCollapsed || variableManagerOpen}
        onSelect={handleSelectModule}
        onToggleSidebar={toggleSidebar}
      />}
      bottomPanel={!variableManagerOpen && activeTab.kind === "ssh" && activeWorkspace ? <SshTerminalLogPanel
        fallback={null}
        collapsed={bottomPanelCollapsed}
        height={bottomPanelHeight}
        onCollapse={() => setBottomPanelCollapsed(true)}
        onHeightChange={setBottomPanelHeight}
        workspaceId={activeWorkspace.id}
      /> : undefined}
      globalToolbar={<AppTitleBar
        activeEnvironmentId={activeEnvironment?.id ?? null}
        activeWorkspace={activeWorkspace}
        environments={workspaceEnvironments}
        endAccessory={TitleBarEnd ? <TitleBarEnd {...extensionContext} /> : undefined}
        extensionContext={extensionContext}
        onActivateWorkspace={handleActivateWorkspace}
        onManageVariables={handleManageVariables}
        onOpenEnvironmentMenu={workspace.refreshWorkspaceEnvironments}
        onSelectEnvironment={workspace.selectEnvironment}
        settingsSections={extensions?.settingsSections}
        workspaceActions={extensions?.workspaceActions}
        workspaceDecoration={extensions?.workspaceDecoration}
        workspaceMenuActions={extensions?.workspaceMenuActions}
        workspaceMenuFooterActions={extensions?.workspaceMenuFooterActions}
        workspaces={workspaceQuery.data?.workspaces ?? []}
      />}
      sidebar={<ModuleSidebar
        activeTab={activeTab}
        apiSidebarContent={apiSidebarContent}
        flowSidebarContent={flowSidebarContent}
        collapsed={sidebarCollapsed || variableManagerOpen}
        databaseSidebarContent={databaseSidebarContent}
        onModuleWidthChange={setModuleSidebarWidth}
        sidebarWidths={sidebarWidths}
        sshSidebarContent={sshSidebarContent}
      />}
      statusBar={<DesktopStatusBar
        activeTab={activeTab}
        workspace={workspace}
        variableManagerOpen={variableManagerOpen}
        databaseStatusBarContent={databaseStatusBarContent}
        rightAccessory={statusBarRightAccessory}
      />}
      main={<DesktopWorkspaceContent
        activeTab={activeTab}
        workspace={workspace}
        navigation={navigation}
        mounts={mounts}
        extensionContext={extensionContext}
        variableDecoration={extensions?.workspaceVariableDecoration}
        onApiSidebarChange={setApiSidebarContent}
        onFlowSidebarChange={setFlowSidebarContent}
        onSshSidebarChange={setSshSidebarContent}
        onDatabaseSidebarChange={setDatabaseSidebarContent}
        onDatabaseStatusBarChange={setDatabaseStatusBarContent}
        onActiveSavedRequestChange={setSelectedApiRequest}
        statusBarRightAccessory={statusBarRightAccessory}
      />}
    />
    <DesktopOverlays
      extensions={extensions}
      extensionContext={extensionContext}
      navigation={navigation}
      commandPaletteOpen={commandPaletteOpen}
      onCommandPaletteOpenChange={setCommandPaletteOpen}
    />
  </FeedbackProvider>;
}

export default DesktopApp;
