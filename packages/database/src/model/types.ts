import type {
  DatabaseCellValue,
  DatabaseConnection,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTable,
} from "@unfour/command-client";

export type TableEditing = {
  pending: boolean;
  primaryKeyColumns: string[];
  onDeleteRow: (primaryKey: DatabaseCellValue[]) => void;
  onInsertRow: (values: DatabaseCellValue[]) => void;
  onUpdateCell: (columnName: string, value: string | null, primaryKey: DatabaseCellValue[]) => void;
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

export type DatabaseResultTab = "results" | "messages" | "logs" | "history";

export type DatabaseWorkspaceTabKind = "sql" | "table-data" | "table-structure" | "view-data";

export type DatabaseWorkspaceTab = {
  connectionId?: string | null;
  id: string;
  kind: DatabaseWorkspaceTabKind;
  loading?: boolean;
  modified?: boolean;
  title: string;
};

export type DatabasePanelState = {
  connections: DatabaseConnection[];
  queryResult: DatabaseQueryResult | null;
  schema?: DatabaseSchema;
  selectedConnection: DatabaseConnection | null;
  selectedTable: DatabaseTable | null;
  tableView: DatabaseTableViewState | null;
};
