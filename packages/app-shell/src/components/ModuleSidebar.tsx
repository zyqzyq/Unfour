import type { ReactNode } from "react";
import type {
  WorkspaceSidebarWidths,
  WorkspaceTab,
} from "@unfour/command-client";
import { Sidebar } from "@unfour/ui";
import {
  MODULE_SIDEBAR_CONFIG,
  normalizeModuleSidebarWidth,
  type ModuleSidebarKind,
} from "@unfour/workspace-core";

export function ModuleSidebar({
  activeTab,
  apiSidebarContent,
  collapsed,
  databaseSidebarContent,
  onModuleWidthChange,
  sshSidebarContent,
  sidebarWidths,
}: {
  activeTab: WorkspaceTab;
  apiSidebarContent?: ReactNode;
  collapsed: boolean;
  databaseSidebarContent?: ReactNode;
  onModuleWidthChange: (kind: ModuleSidebarKind, width: number) => void;
  sshSidebarContent?: ReactNode;
  sidebarWidths: WorkspaceSidebarWidths;
}) {
  if (collapsed) {
    return null;
  }

  const kind = activeTab.kind;
  const config = MODULE_SIDEBAR_CONFIG[kind];
  const width = normalizeModuleSidebarWidth(kind, sidebarWidths[kind]);

  return (
    <Sidebar
      contentClassName={activeTab.kind === "api" ? "overflow-hidden p-0" : undefined}
      maxWidth={config.maxWidth}
      minWidth={config.minWidth}
      onWidthChange={(nextWidth) => onModuleWidthChange(kind, nextWidth)}
      resizable
      width={width}
    >
      {activeTab.kind === "api" && apiSidebarContent}
      {activeTab.kind === "ssh" && sshSidebarContent}
      {activeTab.kind === "database" && databaseSidebarContent}
    </Sidebar>
  );
}
