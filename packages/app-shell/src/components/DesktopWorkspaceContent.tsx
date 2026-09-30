import type { ReactNode } from "react";
import type { WorkspaceTab } from "@unfour/command-client";
import { Button, EmptyState, ErrorState, LoadingState, MainWorkspace, useI18n } from "@unfour/ui";
import type { DesktopAppExtensionContext, DesktopAppExtensions } from "../extensions";
import { ApiClientModule, DatabaseModule, FlowModule, SshTerminalModule, WorkspaceEnvironmentsModule } from "./LazyFeatureModules";
import type { DesktopNavigationController } from "./useDesktopNavigation";
import type { DesktopWorkspaceController } from "./useDesktopWorkspace";
import type { usePersistentFeatureMounts } from "./usePersistentFeatureMounts";

type DesktopWorkspaceContentProps = {
  activeTab: WorkspaceTab;
  workspace: DesktopWorkspaceController;
  navigation: DesktopNavigationController;
  mounts: ReturnType<typeof usePersistentFeatureMounts>;
  extensionContext: DesktopAppExtensionContext;
  variableDecoration?: DesktopAppExtensions["workspaceVariableDecoration"];
  onApiSidebarChange: (content: ReactNode | null) => void;
  onFlowSidebarChange: (content: ReactNode | null) => void;
  onSshSidebarChange: (content: ReactNode | null) => void;
  onDatabaseSidebarChange: (content: ReactNode | null) => void;
  onDatabaseStatusBarChange: (content: ReactNode | null) => void;
  onActiveSavedRequestChange: (requestId: string | null) => void;
  statusBarRightAccessory: ReactNode;
};

function WorkspaceStartupState({ query }: { query: DesktopWorkspaceController["workspaceQuery"] }) {
  const { t } = useI18n();
  if (query.isError) {
    return <ErrorState className="h-full rounded-none border-0">
      <div className="space-y-2" role="alert">
        <p>{t("app.workspace.loadFailed")}</p>
        <Button disabled={query.isFetching} onClick={() => void query.refetch()} size="sm">
          {t("common.actions.retry")}
        </Button>
      </div>
    </ErrorState>;
  }
  if (query.isPending) return <LoadingState className="h-full rounded-none border-0" />;
  return <EmptyState className="h-full rounded-none border-0">{t("app.workspace.createToStart")}</EmptyState>;
}

/** Lazy feature mounts keep their existing identities while visibility changes. */
export function DesktopWorkspaceContent({
  activeTab, workspace, navigation, mounts, extensionContext, variableDecoration: VariableDecoration,
  onApiSidebarChange, onFlowSidebarChange, onSshSidebarChange, onDatabaseSidebarChange,
  onDatabaseStatusBarChange, onActiveSavedRequestChange, statusBarRightAccessory,
}: DesktopWorkspaceContentProps) {
  const { activeWorkspace, workspaceQuery } = workspace;
  const { variableManagerOpen, variableManagerRequest, closeVariableManager, setVariableManagerDirty } = navigation;
  return <MainWorkspace className="[&>section]:p-0" tabBar={null}>
    {!activeWorkspace && <WorkspaceStartupState query={workspaceQuery} />}
    {activeWorkspace && mounts.shouldMountFlow && <div className={activeTab.kind === "flow" && !variableManagerOpen ? "h-full" : "hidden"}>
      <FlowModule key={activeWorkspace.id} workspaceId={activeWorkspace.id} onSidebarContentChange={onFlowSidebarChange} />
    </div>}
    {activeWorkspace && mounts.shouldMountApi && <div className={activeTab.kind === "api" && !variableManagerOpen ? "h-full" : "hidden"}>
      <ApiClientModule
        active={activeTab.kind === "api" && !variableManagerOpen}
        onShellSidebarChange={onApiSidebarChange}
        onActiveSavedRequestChange={onActiveSavedRequestChange}
        openIntent={null}
        workspaceId={activeWorkspace.id}
      />
    </div>}
    {activeWorkspace && mounts.shouldMountSsh && <div className={activeTab.kind === "ssh" && !variableManagerOpen ? "h-full" : "hidden"}>
      <SshTerminalModule
        active={activeTab.kind === "ssh" && !variableManagerOpen}
        onShellSidebarChange={onSshSidebarChange}
        workspaceId={activeWorkspace.id}
      />
    </div>}
    {/* Reuse the DatabasePage's Monaco instance after its first visit to preserve
        its theme and avoid a white repaint when returning to a query tab. */}
    {activeWorkspace && mounts.shouldMountDatabase && <div className={activeTab.kind === "database" && !variableManagerOpen ? "h-full" : "hidden"}>
      <DatabaseModule
        active={activeTab.kind === "database" && !variableManagerOpen}
        onShellSidebarChange={onDatabaseSidebarChange}
        onShellStatusBarChange={onDatabaseStatusBarChange}
        statusBarRightAccessory={statusBarRightAccessory}
        workspaceName={activeWorkspace.name}
        workspaceId={activeWorkspace.id}
      />
    </div>}
    {activeWorkspace && variableManagerOpen && variableManagerRequest && <WorkspaceEnvironmentsModule
      initialEnvironmentId={variableManagerRequest.environmentId}
      key={activeWorkspace.id}
      onClose={closeVariableManager}
      onDirtyChange={setVariableManagerDirty}
      variableDecoration={VariableDecoration ? (variable) => <VariableDecoration {...extensionContext} variable={variable} /> : undefined}
      workspaceId={activeWorkspace.id}
    />}
  </MainWorkspace>;
}
