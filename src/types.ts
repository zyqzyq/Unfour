export type Workspace = {
  id: string;
  name: string;
  isDefault: boolean;
  lastOpenedAt: string | null;
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

export type KeyValue = {
  key: string;
  value: string;
  enabled: boolean;
};

export type WorkspaceEnvironment = {
  workspaceId: string;
  variables: KeyValue[];
  updatedAt: string;
};

export type ApiRequestInput = {
  workspaceId: string;
  name?: string;
  method: string;
  url: string;
  headers: KeyValue[];
  query: KeyValue[];
  body?: string;
  bodyKind: string;
  timeoutMs?: number;
};

export type ApiResponse = {
  historyId: string;
  status: number;
  statusText: string;
  headers: KeyValue[];
  body: string;
  durationMs: number;
};

export type ApiHistoryItem = {
  id: string;
  workspaceId: string;
  name: string | null;
  method: string;
  url: string;
  status: number | null;
  durationMs: number | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type ApiHistoryDetail = {
  id: string;
  workspaceId: string;
  name: string | null;
  method: string;
  url: string;
  requestHeadersJson: string;
  requestQueryJson: string;
  requestBody: string | null;
  status: number | null;
  durationMs: number | null;
  responseHeadersJson: string;
  responseBodyPreview: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type ApiSavedRequest = {
  id: string;
  workspaceId: string;
  name: string;
  method: string;
  url: string;
  headersJson: string;
  queryJson: string;
  body: string | null;
  bodyKind: string;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type DatabaseConnectionInput = {
  id?: string;
  workspaceId: string;
  name: string;
  driver: "sqlite" | "postgres" | "mysql";
  host?: string | null;
  port?: number | null;
  database?: string | null;
  username?: string | null;
  sqlitePath?: string | null;
  credentialRef?: string | null;
};

export type DatabaseConnection = {
  id: string;
  workspaceId: string;
  name: string;
  driver: "sqlite" | "postgres" | "mysql";
  host: string | null;
  port: number | null;
  database: string | null;
  username: string | null;
  sqlitePath: string | null;
  credentialRef: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type DatabaseTestResult = {
  ok: boolean;
  message: string;
  serverVersion: string | null;
};

export type DatabaseSchema = {
  connectionId: string;
  tables: DatabaseTable[];
};

export type DatabaseTable = {
  name: string;
  kind: string;
  columns: DatabaseTableColumn[];
};

export type DatabaseTableColumn = {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
};

export type DatabaseQueryInput = {
  workspaceId: string;
  connectionId: string;
  sql: string;
  limit?: number;
  confirmMutation?: boolean;
};

export type DatabaseBrowseInput = {
  workspaceId: string;
  connectionId: string;
  tableName: string;
  limit?: number;
};

export type DatabaseBrowseResult = {
  sql: string;
  result: DatabaseQueryResult;
};

export type DatabaseQueryResult = {
  columns: DatabaseResultColumn[];
  rows: Array<Array<string | null>>;
  affectedRows: number;
  durationMs: number;
  safety: DatabaseQuerySafety;
};

export type DatabaseResultColumn = {
  name: string;
  dataType: string;
};

export type DatabaseQuerySafety = {
  classification: string;
  requiresConfirmation: boolean;
  confirmed: boolean;
  message: string | null;
};

export type SystemHealth = {
  appName: string;
  storageReady: boolean;
  commandBusReady: boolean;
  aiReservedCapabilities: string[];
  syncStrategy: string;
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
  updatedAt: string;
};
