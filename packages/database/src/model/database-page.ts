import type { ReactNode } from "react";
import type { DatabaseConnection, DatabaseTable, SavedSql } from "@unfour/command-client";

export type DatabasePageProps = {
  active?: boolean;
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  onShellStatusBarChange?: (statusBar: ReactNode | null) => void;
  statusBarRightAccessory?: ReactNode;
  workspaceName?: string;
  workspaceId: string;
};

export function groupSavedSqlByConnection(saved: SavedSql[]) {
  const grouped: Record<string, SavedSql[]> = {};
  for (const item of saved) {
    if (item.connectionId) {
      (grouped[item.connectionId] ??= []).push(item);
    }
  }
  return grouped;
}

export type DatabaseSidebarActions = {
  connect: (connection: DatabaseConnection) => void;
  delete: (connection: DatabaseConnection) => void;
  deleteSavedSql: (item: SavedSql) => void;
  duplicate: (connection: DatabaseConnection) => void;
  designTable: (connectionId: string, table: DatabaseTable) => void;
  disconnect: (connection: DatabaseConnection) => void;
  edit: (connection: DatabaseConnection) => void;
  newConnection: () => void;
  newQuery: (connection?: DatabaseConnection) => void;
  openSavedSql: (item: SavedSql) => void;
  previewTable: (connectionId: string, table: DatabaseTable) => void;
  refresh: () => void;
  refreshSchema: (connection: DatabaseConnection) => void;
  selectConnection: (connection: DatabaseConnection) => void;
  selectTable: (connectionId: string, table: DatabaseTable) => void;
  toggleCatalog: (connectionId: string, catalog: string) => void;
  toggleConnection: (connection: DatabaseConnection) => void;
  useSql: (connectionId: string, sql: string, table?: DatabaseTable) => void;
};
