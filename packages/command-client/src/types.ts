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

export type ApiEnvironment = {
  id: string;
  workspaceId: string;
  name: string;
  variables: KeyValue[];
  isActive: boolean;
  createdAt: string;
  updatedAt: string;
};

export type ApiCollection = {
  id: string;
  workspaceId: string;
  name: string;
  description: string | null;
  folders: string[];
  createdAt: string;
  updatedAt: string;
};

export type ApiRequestInput = {
  workspaceId: string;
  name?: string;
  folderPath?: string | null;
  collectionId?: string | null;
  authJson?: string;
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
  folderPath: string | null;
  collectionId: string | null;
  authJson?: string;
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
  readOnly?: boolean;
};

export type SshAuthKind = "password" | "private-key" | "none";

export type SshConnectionInput = {
  id?: string;
  workspaceId: string;
  name: string;
  host: string;
  port?: number | null;
  username: string;
  authKind: SshAuthKind;
  keyPath?: string | null;
  credentialRef?: string | null;
  /** Plaintext password / key passphrase; stored in the OS keychain on save,
   * never persisted to SQLite. Leave null when editing to keep the saved one. */
  secret?: string | null;
};

export type CredentialCreateInput = {
  workspaceId: string;
  kind: string;
  label: string;
  secret: string;
};

export type CredentialDeleteInput = {
  workspaceId: string;
  credentialRef: string;
};

export type CredentialInspectInput = {
  workspaceId: string;
  credentialRef: string;
};

export type CredentialRotateInput = {
  workspaceId: string;
  credentialRef: string;
  secret: string;
};

export type CredentialMetadata = {
  workspaceId: string;
  kind: string;
  label: string;
  credentialRef: string;
};

export type SshConnection = {
  id: string;
  workspaceId: string;
  name: string;
  host: string;
  port: number;
  username: string;
  authKind: SshAuthKind;
  keyPath: string | null;
  credentialRef: string | null;
  createdAt: string;
  updatedAt: string;
  deletedAt: string | null;
  revision: number;
  syncStatus: string;
  remoteId: string | null;
};

export type SshConnectInput = {
  workspaceId: string;
  connectionId: string;
  cols?: number | null;
  rows?: number | null;
  /**
   * Transient credential override for validating a not-yet-saved secret (e.g.
   * the "test connection" action). When omitted, the saved keychain credential
   * is used. Never persisted.
   */
  secret?: string | null;
};

export type SshSessionInput = {
  workspaceId: string;
  sessionId: string;
  data: string;
};

export type SshResizeInput = {
  workspaceId: string;
  sessionId: string;
  cols: number;
  rows: number;
};

export type SshCloseInput = {
  workspaceId: string;
  sessionId: string;
};

export type SshReconnectCancelInput = {
  workspaceId: string;
  sessionId: string;
};

export type SshLogExportInput = {
  workspaceId: string;
  sessionId: string;
};

export type SshSessionSummary = {
  sessionId: string;
  workspaceId: string;
  connectionId: string;
  status: "connected" | "degraded" | "reconnecting" | "disconnected" | "failed";
  reconnectAttempt: number;
  authKind: SshAuthKind;
  host: string;
  username: string;
  cols: number;
  rows: number;
  createdAt: string;
  updatedAt: string;
};

export type SshSessionEvent = {
  sessionId: string;
  kind: "input" | "output" | "resize" | "close";
  data: string;
  createdAt: string;
};

export type SshLogExport = {
  sessionId: string;
  filename: string;
  content: string;
  lineCount: number;
  redacted: boolean;
};

export type SshHostKeyInput = {
  host: string;
  port: number;
};

export type SshHostFingerprintInfo = {
  host: string;
  port: number;
  fingerprint: string;
  createdAt: string;
};

export type SshKnownHostsImportInput = {
  workspaceId: string;
  content: string;
};

export type SshKnownHostsImportResult = {
  imported: number;
  skipped: number;
  errors: string[];
};

export type SshKnownHostsExportResult = {
  content: string;
  entryCount: number;
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
  readOnly: boolean;
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
  /** Top-level container: PostgreSQL/MySQL database, or null for SQLite. */
  catalog?: string | null;
  /** Nested namespace below the catalog. Only PostgreSQL populates this. */
  schema?: string | null;
  name: string;
  kind: string;
  columns: DatabaseTableColumn[];
};

export type DatabaseTableColumn = {
  name: string;
  dataType: string;
  nullable: boolean;
  primaryKey: boolean;
  defaultValue?: string | null;
};

export type DatabaseIndex = {
  name: string;
  columns: string[];
  unique: boolean;
  primary: boolean;
};

export type DatabaseForeignKey = {
  name: string;
  columns: string[];
  referencedTable: string;
  referencedColumns: string[];
};

export type DatabaseTableStructureInput = {
  workspaceId: string;
  connectionId: string;
  catalog?: string | null;
  schema?: string | null;
  tableName: string;
};

export type DatabaseTableStructure = {
  catalog?: string | null;
  schema?: string | null;
  name: string;
  kind: string;
  columns: DatabaseTableColumn[];
  indexes: DatabaseIndex[];
  foreignKeys: DatabaseForeignKey[];
  ddl?: string | null;
};

export type DatabaseCellValue = {
  column: string;
  value: string | null;
};

export type DatabaseRowMutationInput = {
  workspaceId: string;
  connectionId: string;
  catalog?: string | null;
  schema?: string | null;
  tableName: string;
  operation: "insert" | "update" | "delete";
  values?: DatabaseCellValue[];
  primaryKey?: DatabaseCellValue[];
};

export type DatabaseRowMutationResult = {
  affectedRows: number;
  sql: string;
};

export type DatabaseQueryInput = {
  workspaceId: string;
  connectionId: string;
  sql: string;
  limit?: number;
  confirmMutation?: boolean;
  /** Query context: catalog (PostgreSQL/MySQL database) to run against. */
  catalog?: string | null;
  /** Query context: schema (PostgreSQL) for unqualified name resolution. */
  schema?: string | null;
  /** Optional per-statement timeout in milliseconds; clamped server-side. */
  timeoutMs?: number;
};

export type DbQueryHistoryEntry = {
  id: string;
  workspaceId: string;
  connectionId: string | null;
  connectionName: string;
  sql: string;
  status: "success" | "failed";
  classification: string | null;
  rowCount: number | null;
  affectedRows: number | null;
  durationMs: number | null;
  error: string | null;
  executedAt: string;
};

export type SavedSql = {
  id: string;
  workspaceId: string;
  connectionId: string | null;
  name: string;
  sql: string;
  createdAt: string;
  updatedAt: string;
};

export type SavedSqlInput = {
  id?: string | null;
  workspaceId: string;
  connectionId?: string | null;
  name: string;
  sql: string;
};

export type DatabaseBrowseInput = {
  workspaceId: string;
  connectionId: string;
  catalog?: string | null;
  schema?: string | null;
  tableName: string;
  limit?: number;
  offset?: number;
  orderBy?: string | null;
  orderDescending?: boolean;
  filter?: string | null;
  timeoutMs?: number;
};

export type DatabaseBrowseResult = {
  tableName: string;
  sql: string;
  limit: number;
  offset: number;
  totalRows: number;
  readOnly: boolean;
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
