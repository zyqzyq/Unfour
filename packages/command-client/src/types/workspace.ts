export type WorkspaceEnvironmentType = "dev" | "test" | "prod";

export type WorkspaceMcpPolicy =
  | "auto"
  | "disabled"
  | "read_only"
  | "guarded"
  | "full_access";

export type Workspace = {
  id: string;
  name: string;
  isDefault: boolean;
  lastOpenedAt: string | null;
  environmentType: WorkspaceEnvironmentType;
  mcpPolicy: WorkspaceMcpPolicy;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type WorkspaceState = {
  activeWorkspaceId: string;
  workspaces: Workspace[];
};

export type WorkspaceTab = {
  id: string;
  title: string;
  kind: "api" | "ssh" | "database";
};

export type WorkspaceLayout = {
  workspaceId: string;
  sidebarCollapsed: boolean;
  activeTabId: string;
  tabs: WorkspaceTab[];
  selectedApiRequestId: string | null;
  selectedDatabaseConnectionId: string | null;
  selectedSshConnectionId: string | null;
  sidebarWidth?: number;
  bottomPanelHeight?: number;
  rightInspectorWidth?: number;
  updatedAt: string;
};
