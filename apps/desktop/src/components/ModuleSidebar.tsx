import { Database } from "lucide-react";
import type { ReactNode } from "react";
import {
  DatabaseConnectionTree,
} from "@unfour/database";
import {
  SshConnectionTree,
} from "@unfour/ssh-terminal";
import type {
  DatabaseConnection,
  WorkspaceTab,
} from "@unfour/command-client";
import {
  Sidebar,
  SidebarRow,
  SidebarSection,
  useI18n,
} from "@unfour/ui";

export function ModuleSidebar({
  activeTab,
  activeTabId,
  activeWorkspaceId,
  apiSidebarContent,
  collapsed,
  databaseConnections,
  onSelectDatabaseConnection,
  onWidthChange,
  selectedDatabaseConnectionId,
  setActiveTab,
  width,
}: {
  activeTab: WorkspaceTab;
  activeTabId: string;
  activeWorkspaceId: string;
  apiSidebarContent?: ReactNode;
  collapsed: boolean;
  databaseConnections: DatabaseConnection[];
  onSelectDatabaseConnection: (connection: DatabaseConnection) => void;
  onWidthChange: (width: number) => void;
  selectedDatabaseConnectionId: string | null;
  setActiveTab: (tabId: string) => void;
  width: number;
}) {
  const { t } = useI18n();

  if (collapsed) {
    return null;
  }

  return (
    <Sidebar
      contentClassName={activeTab.kind === "api" ? "overflow-hidden p-0" : undefined}
      onWidthChange={onWidthChange}
      resizable
      width={width}
    >
      {activeTab.kind === "api" && apiSidebarContent}
      {activeTab.kind === "ssh" && (
        <SshConnectionTree
          active={activeTabId === "ssh-main"}
          collapsed={false}
          onOpenTerminal={() => setActiveTab("ssh-main")}
          workspaceId={activeWorkspaceId}
        />
      )}
      {activeTab.kind === "database" && (
        <div className="space-y-3">
          <SidebarSection title={t("app.nav.database")}>
            <div className="space-y-1">
              <SidebarRow
                active={activeTabId === "database-main" && !selectedDatabaseConnectionId}
                onClick={() => setActiveTab("database-main")}
              >
                <Database size={14} />
                <span className="truncate">{t("app.sidebar.sqlWorkspace")}</span>
              </SidebarRow>
              <DatabaseConnectionTree
                connections={databaseConnections}
                onNewQuery={() => setActiveTab("database-main")}
                onSelectConnection={onSelectDatabaseConnection}
                selectedConnectionId={selectedDatabaseConnectionId}
              />
            </div>
          </SidebarSection>
        </div>
      )}
    </Sidebar>
  );
}
