export type DatabaseConnectionInput = {
  id?: string;
  workspaceId: string;
  name: string;
  driver: "sqlite" | "postgres" | "mysql";
  host?: string | null;
  port?: number | null;
  database?: string | null;
  username?: string | null;
  sslMode?: "disable" | "prefer" | "require" | "verify-ca" | "verify-full" | null;
  sqlitePath?: string | null;
  credentialRef?: string | null;
  readOnly?: boolean;
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
  sslMode: "disable" | "prefer" | "require" | "verify-ca" | "verify-full" | null;
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
