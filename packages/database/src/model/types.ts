import type {
  DatabaseCellValue,
  DatabaseConnection,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTable,
} from "@unfour/command-client";

export type TableEditing = {
  canInsert: boolean;
  canUpdateDelete: boolean;
  pending: boolean;
  pendingChanges: PendingTableChange[];
  previewSql: string;
  primaryKeyColumns: string[];
  rowKey: (row: Array<string | null>) => string;
  onApply: () => void;
  onDeleteRow: (row: Array<string | null>, rowKey?: string) => void;
  onInsertRow: (values: DatabaseCellValue[]) => void;
  onRevert: () => void;
  onUpdateCell: (
    row: Array<string | null>,
    columnName: string,
    value: DatabaseCellValue,
    rowKey?: string,
  ) => void;
};

export type PendingTableChange = {
  id: string;
  operation: "insert" | "update" | "delete";
  originalValues: DatabaseCellValue[];
  primaryKey: DatabaseCellValue[];
  rowKey: string;
  values: DatabaseCellValue[];
};

export type DatabaseTableViewState = {
  pageIndex: number;
  pageSize: number;
  readOnly: boolean;
  tableName: string;
  totalRows: number;
};

export type DatabaseConnectionStatus = "disconnected" | "connecting" | "connected" | "failed";

export type DatabaseConnectionSessionState = {
  message?: string | null;
  serverVersion?: string | null;
  status: DatabaseConnectionStatus;
  updatedAt?: string;
};

export type SqlHistoryEntry = {
  affectedRows?: number;
  classification?: string;
  connectionId: string | null;
  connectionName: string;
  durationMs?: number;
  error?: string;
  executedAt: string;
  id: string;
  rowCount?: number;
  sql: string;
  status: "success" | "failed";
};

// Explicit execution context for a query window. `connectionId` identifies the
// datasource; `catalog`/`schema` scope where unqualified names resolve and which
// database the statement runs against (applied server-side before execution).
export type QueryContext = {
  connectionId: string | null;
  catalog: string | null;
  schema: string | null;
};

export type DatabaseResultTab = "results" | "messages" | "logs" | "history";

/** Options for Run Current / Run All from the SQL editor. */
export type RunSqlOptions = {
  cursorOffset?: number;
  mode?: "current" | "all";
  /** Continue a paused batch after CONFIRMATION_REQUIRED. */
  resume?: boolean;
  sql?: string;
};

export type DatabaseStructureTab = "ddl" | "indexes" | "constraints" | "properties";

export type TableQueryState = {
  orderBy: string | null;
  orderDescending: boolean;
  filter: string;
};

export const emptyTableQuery: TableQueryState = {
  orderBy: null,
  orderDescending: false,
  filter: "",
};

// Object-level workspace tab ids are dynamic so multiple query/table objects
// can stay open at once.
export type DatabaseWorkspaceTabId = string;

export type TableSegment = "data" | "structure";

export type DatabaseWorkspaceTabKind = "query" | "table";

export type DatabaseQueryWorkspaceTab = {
  activeResultIndex: number;
  catalog: string | null;
  connectionId: string | null;
  error: unknown;
  id: DatabaseWorkspaceTabId;
  kind: "query";
  loading?: boolean;
  pendingConfirmation: boolean;
  /** Active result set; mirrors `results[activeResultIndex]` when present. */
  result: DatabaseQueryResult | null;
  /** All result sets from the latest Run / Run All (SELECT and non-SELECT). */
  results: DatabaseQueryResult[];
  resultTab: DatabaseResultTab;
  schema: string | null;
  sql: string;
  title: string;
};

export type DatabaseTableWorkspaceTab = {
  connectionId: string;
  error: unknown;
  id: DatabaseWorkspaceTabId;
  kind: "table";
  loading?: boolean;
  pendingChanges: PendingTableChange[];
  queryResult: DatabaseQueryResult | null;
  segment: TableSegment;
  structureTab: DatabaseStructureTab;
  table: DatabaseTable;
  tableQuery: TableQueryState;
  tableView: DatabaseTableViewState | null;
  title: string;
};

export type DatabaseWorkspaceTab = DatabaseQueryWorkspaceTab | DatabaseTableWorkspaceTab;

export type DatabasePanelState = {
  connections: DatabaseConnection[];
  queryResult: DatabaseQueryResult | null;
  schema?: DatabaseSchema;
  selectedConnection: DatabaseConnection | null;
  selectedTable: DatabaseTable | null;
  tableView: DatabaseTableViewState | null;
};
