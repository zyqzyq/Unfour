import {
  Database,
  PanelLeftClose,
  PanelLeftOpen,
} from "lucide-react";
import {
  ApiCollectionTree,
  type ApiOpenIntent,
} from "@unfour/api-client";
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
  IconButton,
  Sidebar,
  SidebarHeader,
  SidebarRow,
  SidebarSection,
} from "@unfour/ui";
import { ModuleSwitcher } from "./ModuleSwitcher";

export function ModuleSidebar({
  activeTab,
  activeTabId,
  activeWorkspaceId,
  collapsed,
  databaseConnections,
  onSelectApiRequest,
  onOpenApiIntent,
  onSelectDatabaseConnection,
  onToggle,
  onWidthChange,
  selectedApiRequestId,
  selectedDatabaseConnectionId,
  setActiveTab,
  setSelectedApiRequest,
  width,
}: {
  activeTab: WorkspaceTab;
  activeTabId: string;
  activeWorkspaceId: string;
  collapsed: boolean;
  databaseConnections: DatabaseConnection[];
  onSelectApiRequest: (requestId: string) => void;
  onOpenApiIntent: (intent: ApiOpenIntent) => void;
  onSelectDatabaseConnection: (connection: DatabaseConnection) => void;
  onToggle: () => void;
  onWidthChange: (width: number) => void;
  selectedApiRequestId: string | null;
  selectedDatabaseConnectionId: string | null;
  setActiveTab: (tabId: string) => void;
  setSelectedApiRequest: (requestId: string | null) => void;
  width: number;
}) {
  return (
    <Sidebar
      collapsed={collapsed}
      className="bg-[var(--u-color-surface-subtle)]"
      header={
        <SidebarHeader className="h-auto flex-col items-stretch gap-2 p-2">
          <div className="flex w-full justify-end">
            <IconButton
              className={collapsed ? "w-full" : "shrink-0"}
              label={collapsed ? "Expand sidebar" : "Collapse sidebar"}
              onClick={onToggle}
            >
              {collapsed ? <PanelLeftOpen size={15} /> : <PanelLeftClose size={15} />}
            </IconButton>
          </div>
          <ModuleSwitcher
            activeKind={activeTab.kind}
            collapsed={collapsed}
            onSelect={(tabId) => setActiveTab(tabId)}
          />
        </SidebarHeader>
      }
      onWidthChange={onWidthChange}
      resizable
      width={width}
    >
      {activeTab.kind === "api" && (
        <ApiCollectionTree
          active={activeTabId === "api-main"}
          collapsed={collapsed}
          onOpenClient={() => {
            setSelectedApiRequest(null);
            onOpenApiIntent({ kind: "new", nonce: Date.now() });
            setActiveTab("api-main");
          }}
          onOpenIntent={(intent) => {
            if (intent.kind === "saved") {
              onSelectApiRequest(intent.requestId);
            }
            onOpenApiIntent(intent);
          }}
          selectedId={selectedApiRequestId}
          workspaceId={activeWorkspaceId}
        />
      )}
      {activeTab.kind === "ssh" && (
        <SshConnectionTree
          active={activeTabId === "ssh-main"}
          collapsed={collapsed}
          onOpenTerminal={() => setActiveTab("ssh-main")}
          workspaceId={activeWorkspaceId}
        />
      )}
      {activeTab.kind === "database" && (
        <div className="space-y-3">
          <ResourceGroup collapsed={collapsed} title="Database">
            <SidebarAction
              collapsed={collapsed}
              icon={<Database size={14} />}
              label="SQL Workspace"
              onClick={() => setActiveTab("database-main")}
              selected={activeTabId === "database-main" && (collapsed || !selectedDatabaseConnectionId)}
            />
            {!collapsed && (
              <DatabaseConnectionTree
                connections={databaseConnections}
                onNewQuery={() => setActiveTab("database-main")}
                onSelectConnection={onSelectDatabaseConnection}
                selectedConnectionId={selectedDatabaseConnectionId}
              />
            )}
          </ResourceGroup>
        </div>
      )}
    </Sidebar>
  );
}

function ResourceGroup({
  children,
  collapsed,
  title,
}: {
  children: React.ReactNode;
  collapsed: boolean;
  title: string;
}) {
  return (
    <SidebarSection title={collapsed ? undefined : title}>
      <div className="space-y-1">{children}</div>
    </SidebarSection>
  );
}

function SidebarAction({
  collapsed,
  icon,
  label,
  onClick,
  selected,
}: {
  collapsed: boolean;
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
  selected: boolean;
}) {
  return (
    <SidebarRow active={selected} onClick={onClick}>
      {icon}
      {!collapsed && <span className="truncate">{label}</span>}
    </SidebarRow>
  );
}
