import { useEffect, useLayoutEffect, useMemo, useRef } from "react";
import type { DatabaseConnection, DatabaseSchema, SavedSql, DatabaseTable } from "@unfour/command-client";
import { DatabaseSidebar } from "../components/DatabaseSidebar";
import { DatabaseStatusBar } from "../components/DatabaseStatusBar";
import { groupSavedSqlByConnection, type DatabasePageProps, type DatabaseSidebarActions } from "../model/database-page";
import type { DatabaseConnectionSessionState } from "../model/types";

type DatabaseShellIntegrationOptions = DatabasePageProps & {
  catalogNamesByConn: Record<string, string[]>;
  connections: DatabaseConnection[];
  connectionStates: Record<string, DatabaseConnectionSessionState>;
  executePending: boolean;
  savedSql: SavedSql[];
  selectedConnectionId: string | null;
  selectedTableId: string | null;
  sidebarActions: DatabaseSidebarActions;
  toolbarConnection: DatabaseConnection | null;
  toolbarSession: DatabaseConnectionSessionState | undefined;
  treeErrors: Record<string, string>;
  treeLoadingKeys: string[];
  treeSchemaCache: Record<string, DatabaseSchema>;
};

export function useDatabaseShellIntegration({
  active = true,
  catalogNamesByConn,
  connections,
  connectionStates,
  executePending,
  onShellSidebarChange,
  onShellStatusBarChange,
  savedSql,
  selectedConnectionId,
  selectedTableId,
  sidebarActions,
  statusBarRightAccessory,
  toolbarConnection,
  toolbarSession,
  treeErrors,
  treeLoadingKeys,
  treeSchemaCache,
  workspaceId,
  workspaceName,
}: DatabaseShellIntegrationOptions) {
  const savedSqlByConnection = useMemo(() => groupSavedSqlByConnection(savedSql), [savedSql]);
  // stable callback identities and only re-render on data changes.
  const sidebarActionsRef = useRef<DatabaseSidebarActions | null>(null);
  useLayoutEffect(() => {
    sidebarActionsRef.current = sidebarActions;
  });

  const sidebarHandlers = useMemo(
    () => ({
      onConnect: (connection: DatabaseConnection) => sidebarActionsRef.current?.connect(connection),
      onDesignTable: (connectionId: string, table: DatabaseTable) =>
        sidebarActionsRef.current?.designTable(connectionId, table),
      onDeleteConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.delete(connection),
      onDeleteSavedSql: (item: SavedSql) => sidebarActionsRef.current?.deleteSavedSql(item),
      onDuplicateConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.duplicate(connection),
      onDisconnect: (connection: DatabaseConnection) => sidebarActionsRef.current?.disconnect(connection),
      onEditConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.edit(connection),
      onNewConnection: () => sidebarActionsRef.current?.newConnection(),
      onNewQuery: (connection?: DatabaseConnection, catalog?: string) => sidebarActionsRef.current?.newQuery(connection, catalog),
      onOpenSavedSql: (item: SavedSql) => sidebarActionsRef.current?.openSavedSql(item),
      onPreviewTable: (connectionId: string, table: DatabaseTable) =>
        sidebarActionsRef.current?.previewTable(connectionId, table),
      onRefresh: () => sidebarActionsRef.current?.refresh(),
      onRefreshSchema: (connection: DatabaseConnection) => sidebarActionsRef.current?.refreshSchema(connection),
      onSelectConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.selectConnection(connection),
      onSelectTable: (connectionId: string, table: DatabaseTable) =>
        sidebarActionsRef.current?.selectTable(connectionId, table),
      onToggleCatalog: (connectionId: string, catalog: string) =>
        sidebarActionsRef.current?.toggleCatalog(connectionId, catalog),
      onToggleConnection: (connection: DatabaseConnection) =>
        sidebarActionsRef.current?.toggleConnection(connection),
      onUseSql: (connectionId: string, sql: string, table?: DatabaseTable) =>
        sidebarActionsRef.current?.useSql(connectionId, sql, table),
    }),
    [],
  );

  const shellSidebar = useMemo(
    () => (
      <DatabaseSidebar
        catalogNamesByConnection={catalogNamesByConn}
        connectionStates={connectionStates}
        connections={connections}
        loadErrors={treeErrors}
        loadingKeys={treeLoadingKeys}
        savedSqlByConnection={savedSqlByConnection}
        schemaCache={treeSchemaCache}
        selectedConnectionId={selectedConnectionId}
        selectedTableId={selectedTableId}
        {...sidebarHandlers}
      />
    ),
    [
      catalogNamesByConn,
      connectionStates,
      connections,
      savedSqlByConnection,
      selectedConnectionId,
      selectedTableId,
      sidebarHandlers,
      treeErrors,
      treeLoadingKeys,
      treeSchemaCache,
    ],
  );

  useEffect(() => {
    if (!active || !onShellSidebarChange) {
      return;
    }
    onShellSidebarChange(shellSidebar);
    return () => onShellSidebarChange(null);
  }, [active, onShellSidebarChange, shellSidebar]);

  const shellStatusBar = useMemo(
    () => (
      <DatabaseStatusBar
        connection={toolbarConnection}
        executing={executePending}
        rightAccessory={statusBarRightAccessory}
        session={toolbarSession}
        workspaceName={workspaceName ?? workspaceId}
      />
    ),
    [
      executePending,
      toolbarConnection,
      toolbarSession,
      statusBarRightAccessory,
      workspaceId,
      workspaceName,
    ],
  );

  useEffect(() => {
    if (!active || !onShellStatusBarChange) {
      return;
    }
    onShellStatusBarChange(shellStatusBar);
    return () => onShellStatusBarChange(null);
  }, [active, onShellStatusBarChange, shellStatusBar]);
}
