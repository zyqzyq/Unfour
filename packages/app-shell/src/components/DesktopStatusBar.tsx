import type { ReactNode } from "react";
import type { WorkspaceTab } from "@unfour/command-client";
import { SshTerminalStatusBar, WorkspaceEnvironmentsModuleStatusBar } from "./LazyFeatureModules";
import { StatusBarPlaceholder } from "./StatusBarPlaceholder";
import type { DesktopWorkspaceController } from "./useDesktopWorkspace";

export function DesktopStatusBar({ activeTab, workspace, variableManagerOpen, databaseStatusBarContent, rightAccessory }: {
  activeTab: WorkspaceTab;
  workspace: DesktopWorkspaceController;
  variableManagerOpen: boolean;
  databaseStatusBarContent: ReactNode;
  rightAccessory: ReactNode;
}) {
  const { activeWorkspace, healthQuery } = workspace;
  const placeholder = <StatusBarPlaceholder
    activeTab={activeTab}
    activeWorkspace={activeWorkspace}
    healthReady={healthQuery.data?.storageReady}
    healthError={healthQuery.isError}
    rightAccessory={rightAccessory}
  />;
  if (variableManagerOpen && activeWorkspace) {
    return <WorkspaceEnvironmentsModuleStatusBar fallback={placeholder} workspaceName={activeWorkspace.name} />;
  }
  if (activeTab.kind === "ssh" && activeWorkspace) {
    return <SshTerminalStatusBar
      fallback={placeholder}
      rightAccessory={rightAccessory}
      workspaceId={activeWorkspace.id}
      workspaceName={activeWorkspace.name}
    />;
  }
  if (activeTab.kind === "database" && databaseStatusBarContent) return databaseStatusBarContent;
  return placeholder;
}
