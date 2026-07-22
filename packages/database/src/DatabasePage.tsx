import { useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createCredential,
  deleteDatabaseConnection,
  rotateCredential,
  saveDatabaseConnection,
  testDatabaseConnection,
  testDatabaseConnectionInput,
} from "@unfour/command-client";
import type {
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseSchema,
  DatabaseTable,
  DatabaseTestResult,
  SavedSql,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import {
  ConfirmDialog,
  useI18n,
} from "@unfour/ui";
import { DatabaseSidebar } from "./components/DatabaseSidebar";
import { DatabaseTestResultDialog } from "./components/DatabaseTestResultDialog";
import { DatabaseModuleToolbar } from "./components/DatabaseModuleToolbar";
import { DatabaseStatusBar } from "./components/DatabaseStatusBar";
import { DatabaseConnectionErrorBanner } from "./components/DatabaseConnectionErrorBanner";
import { DatabaseConnectionDialog } from "./components/DatabaseConnectionDialog";
import { DatabaseWorkspace } from "./components/DatabaseWorkspace";
import { useDatabaseConnections } from "./hooks/useDatabaseConnections";
import { useDatabaseWorkspaceController } from "./hooks/useDatabaseWorkspaceController";
import { useDatabaseTabs } from "./hooks/useDatabaseTabs";
import { useDatabaseCatalogs } from "./hooks/useDatabaseCatalogs";
import { useQueryHistory } from "./hooks/useQueryHistory";
import { useSavedSql } from "./hooks/useSavedSql";
import { useSchemaTree } from "./hooks/useSchemaTree";
import { useTableStructure } from "./hooks/useTableStructure";
import { buildDatabaseTree, databaseTableTreeId } from "./model/database-tree";
import { normalizeQueryContext } from "./model/database-query-context";
import { groupSavedSqlByConnection, type DatabasePageProps } from "./model/database-page";
import { EMPTY_CONNECTION_STATES, useDatabaseConnectionStore } from "./model/database-connection-state";
import { createTableEditing } from "./model/table-editing";
import type {
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  SqlHistoryEntry,
  TableEditing,
} from "./model/types";
import { formatDatabaseError } from "./result-utils";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;
const MAX_HISTORY_ENTRIES = 25;

export function DatabasePage({
  onShellSidebarChange,
  onShellStatusBarChange,
  statusBarRightAccessory,
  workspaceName,
  workspaceId,
}: DatabasePageProps) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const {
    selectedDatabaseConnectionId: selectedConnectionId,
    setSelectedDatabaseConnection,
  } = useWorkspaceStore();
  const databaseTabs = useDatabaseTabs({
    formatQueryTitle: (index) => t("database.editor.queryTitle", { index }),
    workspaceId,
  });
  const [editorOpen, setEditorOpen] = useState(false);
  const connectionStates = useDatabaseConnectionStore(
    (state) => state.byWorkspace[workspaceId] ?? EMPTY_CONNECTION_STATES,
  );
  const setConnectionStateAction = useDatabaseConnectionStore((state) => state.setConnectionState);
  const pruneConnectionsAction = useDatabaseConnectionStore((state) => state.pruneConnections);
  const removeConnectionAction = useDatabaseConnectionStore((state) => state.removeConnection);
  // Bind the workspace id so the existing call sites keep their original
  // `(connectionId, patch)` / `(connectionId)` signatures.
  const setConnectionState = (connectionId: string, patch: Partial<DatabaseConnectionSessionState>) =>
    setConnectionStateAction(workspaceId, connectionId, patch);
  const removeConnection = (connectionId: string) => removeConnectionAction(workspaceId, connectionId);
  const [testResult, setTestResult] = useState<DatabaseTestResult | null>(null);
  const [queryHistory, setQueryHistory] = useState<SqlHistoryEntry[]>([]);
  const [selectedTable, setSelectedTable] = useState<DatabaseTable | null>(null);
  // Per-connection tree data so multiple connections can be browsed at once.
  // catalogNamesByConn: connectionId -> database names (PostgreSQL/MySQL).
  // treeSchemaCache: `${connectionId}::${catalog}` -> that database's schema
  // ("" catalog for SQLite). Both are populated lazily as nodes are expanded;
  // the selected connection's data is fed in from its own queries.
  const [catalogNamesByConn, setCatalogNamesByConn] = useState<Record<string, string[]>>({});
  const [treeSchemaCache, setTreeSchemaCache] = useState<Record<string, DatabaseSchema>>({});
  const [treeLoadingKeys, setTreeLoadingKeys] = useState<string[]>([]);
  const [treeErrors, setTreeErrors] = useState<Record<string, string>>({});
  const [password, setPassword] = useState("");
  const [form, setForm] = useState<DatabaseConnectionInput>({
    workspaceId,
    name: "",
    driver: "sqlite",
    sqlitePath: "",
  });

  const connectionsQuery = useDatabaseConnections(workspaceId);
  const queryHistoryQuery = useQueryHistory(workspaceId, MAX_HISTORY_ENTRIES);
  const savedSqlQuery = useSavedSql(workspaceId);
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
  // Group saved SQL by its owning connection id so the sidebar tree can render
  // each connection's snippets under a dedicated "Saved Queries" branch.
  const savedSqlByConnection = useMemo(
    () => groupSavedSqlByConnection(savedSqlQuery.saved),
    [savedSqlQuery.saved],
  );
  const prevSelectedConnectionIdRef = useRef(selectedConnectionId);
  const activeTab = databaseTabs.activeTab;
  const activeQueryTab = activeTab?.kind === "query" ? activeTab : null;
  const activeTableTab = activeTab?.kind === "table" ? activeTab : null;
  const selectedSession = selectedConnectionId ? connectionStates[selectedConnectionId] : undefined;
  const selectedConnectionStatus: DatabaseConnectionStatus = selectedSession?.status ?? "disconnected";
  const schemaEnabled = Boolean(
    selectedConnection &&
      (selectedConnectionStatus === "connecting" || selectedConnectionStatus === "connected"),
  );
  // The initial schema load fetches the connected database (PostgreSQL) or every
  // database (MySQL, which can list them in one call). Other PostgreSQL databases
  // are fetched lazily on expand and cached in schemasByCatalog.
  const schemaQuery = useSchemaTree({
    connection: selectedConnection,
    connectionId: selectedConnectionId,
    enabled: schemaEnabled,
    workspaceId,
  });
  const catalogsQuery = useDatabaseCatalogs({
    connection: selectedConnection,
    connectionId: selectedConnectionId,
    enabled: schemaEnabled,
    workspaceId,
  });
  const visibleSchema = schemaEnabled ? schemaQuery.data : undefined;
  const treeModel = useMemo(
    () => (visibleSchema ? buildDatabaseTree(visibleSchema.tables) : null),
    [visibleSchema],
  );
  // Catalog (database) choices for the query context: the server's database
  // list (so PostgreSQL can browse beyond the loaded one) merged with any
  // catalogs present in the loaded schema. Empty for SQLite.
  const catalogOptions = useMemo(() => {
    const merged = new Set<string>();
    for (const name of catalogsQuery.data ?? []) {
      if (name) {
        merged.add(name);
      }
    }
    for (const catalog of treeModel?.catalogs ?? []) {
      if (catalog.key) {
        merged.add(catalog.key);
      }
    }
    return [...merged];
  }, [catalogsQuery.data, treeModel]);
  // Schema choices for the active catalog. Empty unless the catalog nests
  // schemas (PostgreSQL).
  const schemaOptions = useMemo(() => {
    if (!treeModel) {
      return [];
    }
    const activeCatalog =
      treeModel.catalogs.find((catalog) => catalog.key === activeQueryTab?.catalog) ??
      treeModel.catalogs[0];
    if (!activeCatalog?.hasSchemaLevel) {
      return [];
    }
    return activeCatalog.schemas.map((schema) => schema.key).filter((key) => key !== "");
  }, [treeModel, activeQueryTab?.catalog]);
  const structureEnabled = Boolean(
    activeTableTab &&
      activeTableTab.segment === "structure" &&
      (connectionStates[activeTableTab.connectionId]?.status === "connecting" ||
        connectionStates[activeTableTab.connectionId]?.status === "connected"),
  );
  const structureQuery = useTableStructure({
    connectionId: activeTableTab?.connectionId ?? null,
    enabled: structureEnabled,
    table: activeTableTab?.table ?? null,
    workspaceId,
  });
  const selectedTableId =
    selectedConnectionId && selectedTable ? databaseTableTreeId(selectedConnectionId, selectedTable) : null;

  useEffect(() => {
    if (!activeTab) {
      return;
    }
    setSelectedDatabaseConnection(activeTab.connectionId);
    // eslint-disable-next-line react-hooks/set-state-in-effect -- active workspace tab drives shell/tree selection after tab changes
    setSelectedTable(activeTab.kind === "table" ? activeTab.table : null);
  }, [activeTab?.id, setSelectedDatabaseConnection]);

  useEffect(() => {
    if (selectedConnectionId && !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedDatabaseConnection(null);
      // eslint-disable-next-line react-hooks/set-state-in-effect -- clearing derived state when parent selection is removed
      setSelectedTable(null);
    }

    // Drop state for connections that no longer exist. Read from the store
    // directly (not a hook selector) so this effect does not re-run on every
    // connection-state change and loop.
    const liveIds = new Set(connections.map((connection) => connection.id));
    const current = useDatabaseConnectionStore.getState().byWorkspace[workspaceId] ?? {};
    if (Object.keys(current).some((id) => !liveIds.has(id))) {
      pruneConnectionsAction(workspaceId, liveIds);
    }
  }, [connections, selectedConnectionId, setSelectedDatabaseConnection, workspaceId, pruneConnectionsAction]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- syncing persisted query history into local optimistic UI state
    setQueryHistory(queryHistoryQuery.entries.slice(0, MAX_HISTORY_ENTRIES));
  }, [queryHistoryQuery.entries]);

  // Sync form state when the selected connection changes (render-time adjustment pattern).
  if (selectedConnectionId !== prevSelectedConnectionIdRef.current) {
    prevSelectedConnectionIdRef.current = selectedConnectionId;
    setPassword("");
    if (selectedConnection) {
      setForm({
        id: selectedConnection.id,
        workspaceId,
        name: selectedConnection.name,
        driver: selectedConnection.driver,
        host: selectedConnection.host,
        port: selectedConnection.port,
        database: selectedConnection.database,
        username: selectedConnection.username,
        sslMode: selectedConnection.sslMode,
        sqlitePath: selectedConnection.sqlitePath,
        credentialRef: selectedConnection.credentialRef,
        readOnly: selectedConnection.readOnly,
      });
      setTestResult(null);
    }
  }

  useEffect(() => {
    if (!selectedConnectionId || !schemaEnabled || !schemaQuery.data) {
      return;
    }

    setConnectionState(selectedConnectionId, {
      message: t("database.connection.tableCountLoaded", {
        count: schemaQuery.data.tables.length,
      }),
      status: "connected",
    });
  }, [schemaEnabled, schemaQuery.data, selectedConnectionId, t]);

  useEffect(() => {
    if (!selectedConnectionId || !schemaEnabled || !schemaQuery.error) {
      return;
    }

    setConnectionState(selectedConnectionId, {
      message: formatDatabaseError(schemaQuery.error),
      status: "failed",
    });
  }, [schemaEnabled, schemaQuery.error, selectedConnectionId]);

  // Feed the selected connection's database list into the per-connection cache
  // so its tree renders without a manual expand.
  useEffect(() => {
    if (!selectedConnectionId || !catalogsQuery.data) {
      return;
    }
    const names = catalogsQuery.data;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- mirroring the selected connection's loaded catalogs into the shared cache
    setCatalogNamesByConn((prev) => ({ ...prev, [selectedConnectionId]: names }));
  }, [selectedConnectionId, catalogsQuery.data]);

  // Eagerly load the first tree level (database list for PostgreSQL/MySQL, file
  // schema for SQLite) for every connected connection. Only the active
  // connection loads through its own queries; without this a second connected
  // connection would sit empty until manually expanded.
  useEffect(() => {
    for (const connection of connections) {
      const status = connectionStates[connection.id]?.status;
      if (status === "connected" || status === "connecting") {
        loadConnectionRoot(connection);
      }
    }
    // loadConnectionRoot is recreated each render and guards against duplicate
    // fetches internally, so it is intentionally excluded from the deps.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connections, connectionStates]);

  // Feed the selected connection's loaded schema into the cache, grouped by
  // catalog (the connected database for PostgreSQL, every database for MySQL,
  // the file for SQLite under the "" catalog key).
  const selectedSchemaData = schemaQuery.data;
  useEffect(() => {
    if (!selectedConnectionId || !selectedSchemaData) {
      return;
    }
    const grouped = new Map<string, DatabaseTable[]>();
    for (const table of selectedSchemaData.tables) {
      const key = table.catalog ?? "";
      grouped.set(key, [...(grouped.get(key) ?? []), table]);
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- mirroring the selected connection's loaded schema into the shared cache
    setTreeSchemaCache((prev) => {
      const next = { ...prev };
      if (grouped.size === 0) {
        next[`${selectedConnectionId}::`] = selectedSchemaData;
      }
      for (const [catalog, tables] of grouped) {
        next[`${selectedConnectionId}::${catalog}`] = {
          connectionId: selectedConnectionId,
          tables,
        };
      }
      return next;
    });
  }, [selectedConnectionId, selectedSchemaData]);

  // Keep the active query tab pointed at a valid catalog/schema as the schema
  // loads or changes. Preserves a still-valid user selection; otherwise falls
  // back to the first catalog and (for PostgreSQL) its first schema.
  useEffect(() => {
    if (!treeModel || !activeQueryTab) {
      return;
    }
    const next = normalizeQueryContext(activeQueryTab, treeModel);
    if (next.catalog === activeQueryTab.catalog && next.schema === activeQueryTab.schema) {
      return;
    }
    databaseTabs.updateQueryTab(activeQueryTab.id, next);
  }, [treeModel, activeQueryTab?.id, activeQueryTab?.catalog, activeQueryTab?.schema]);

  const saveMutation = useMutation({
    mutationFn: async ({ input, secret }: { input: DatabaseConnectionInput; secret: string }) => {
      let credentialRef = input.credentialRef ?? null;
      // Non-SQLite drivers persist the password through SecretStore and store
      // only the returned reference. An empty secret while editing keeps the
      // existing credential untouched.
      if (input.driver !== "sqlite" && secret.trim()) {
        if (credentialRef) {
          await rotateCredential({ workspaceId, credentialRef, secret });
        } else {
          const metadata = await createCredential({
            workspaceId,
            kind: "database",
            label: input.name,
            secret,
          });
          credentialRef = metadata.credentialRef;
        }
      }
      return saveDatabaseConnection({ ...input, credentialRef });
    },
    onSuccess: (connection) => {
      setPassword("");
      setSelectedDatabaseConnection(connection.id);
      setEditorOpen(false);
      setConnectionState(connection.id, {
        message: t("database.connection.savedBrowseSchema"),
        status: "disconnected",
      });
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const [deleteConfirm, setDeleteConfirm] = useState<DatabaseConnection | null>(null);
  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteDatabaseConnection(workspaceId, connectionId),
    onSuccess: (_result, connectionId) => {
      removeConnection(connectionId);
      // Only reset the active workspace when the deleted connection was the one
      // in use; deleting another connection from the context menu must not clear
      // the current query or table view.
      if (connectionId === selectedConnectionId) {
        setSelectedDatabaseConnection(null);
        setTestResult(null);
        setSelectedTable(null);
      }
      databaseTabs.removeConnectionTabs(connectionId);
      setDeleteConfirm(null);
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  // Clone a connection into a new record. The stored credential is shared by
  // reusing its reference (the plaintext secret is never exposed to the client),
  // so the copy can connect immediately without re-entering the password.
  const duplicateMutation = useMutation({
    mutationFn: (connection: DatabaseConnection) =>
      saveDatabaseConnection({
        workspaceId,
        name: t("database.tree.duplicateName", { name: connection.name }),
        driver: connection.driver,
        host: connection.host,
        port: connection.port,
        database: connection.database,
        username: connection.username,
        sslMode: connection.sslMode,
        sqlitePath: connection.sqlitePath,
        credentialRef: connection.credentialRef,
        readOnly: connection.readOnly,
      }),
    onSuccess: (created) => {
      setSelectedDatabaseConnection(created.id);
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const testMutation = useMutation({
    mutationFn: (connectionId: string) => testDatabaseConnection(workspaceId, connectionId),
    onMutate: (connectionId) => {
      setConnectionState(connectionId, {
        message: t("common.actions.connecting"),
        status: "connecting",
      });
    },
    onError: (error, connectionId) => {
      setTestResult(null);
      setConnectionState(connectionId, {
        message: formatDatabaseError(error),
        status: "failed",
      });
    },
    onSuccess: (result, connectionId) => {
      setTestResult(result);
      setConnectionState(connectionId, {
        message: result.message,
        serverVersion: result.serverVersion,
        status: result.ok ? "connected" : "failed",
      });
      if (result.ok) {
        queryClient.invalidateQueries({ queryKey: ["database-schema", workspaceId, connectionId] });
      }
    },
  });

  // Validate a connection from the dialog form without persisting it. Used by
  // the "Test connection" button, which must work for brand-new (unsaved)
  // connections. Unlike `testMutation` (by saved id, which also opens a
  // session), this only checks connectivity and leaves state disconnected.
  const testInputMutation = useMutation({
    mutationFn: ({ input, secret }: { input: DatabaseConnectionInput; secret: string | null }) =>
      testDatabaseConnectionInput(input, secret),
    onSuccess: (result) => setTestResult(result),
    onError: (error) =>
      setTestResult({ ok: false, message: formatDatabaseError(error), serverVersion: null }),
  });


  const {
    applyTableFilter,
    applyPendingTableChanges,
    applyTableSort,
    browseMutation,
    browseTablePage,
    canTest,
    changeQueryContext,
    clearQueryHistory,
    clearSql,
    connectConnection,
    deleteSavedSql,
    designTable,
    disconnectConnection,
    handleEditConnection,
    handleNewConnection,
    handleSelectResultTab,
    handleSelectStructureTab,
    handleSelectTableSegment,
    handleTablePageChange,
    loadCatalogSchema,
    loadConnectionRoot,
    loadHistoryEntry,
    loadSqlIntoEditor,
    openSavedSql,
    previewSelectedTable,
    refreshActiveSchema,
    refreshConnectionSchema,
    refreshConnectionsAndSchema,
    rowMutation,
    runSql,
    selectConnection,
    selectDatabaseTab,
    selectQueryConnection,
    selectQueryResult,
    selectTable,
    showQueryHistory,
    sqlRunning,
    startNewQuery,
    stopQuery,
    submitConnection,
    testConnectionInput,
    updateActiveSql,
    updateForm,
  } = useDatabaseWorkspaceController({
    activeQueryTab,
    activeTableTab,
    catalogNamesByConn,
    connectionStates,
    connections,
    databaseTabs,
    form,
    maxHistoryEntries: MAX_HISTORY_ENTRIES,
    password,
    queryClient,
    queryHistoryQuery,
    saveMutation,
    savedSqlQuery,
    selectedConnection,
    selectedConnectionId,
    selectedConnectionStatus,
    selectedTable,
    setCatalogNamesByConn,
    setConnectionState,
    setEditorOpen,
    setForm,
    setPassword,
    setQueryHistory,
    setSelectedDatabaseConnection,
    setSelectedTable,
    setTestResult,
    setTreeErrors,
    setTreeLoadingKeys,
    setTreeSchemaCache,
    t,
    testInputMutation,
    testMutation,
    treeLoadingKeys,
    treeSchemaCache,
    workspaceId,
  });

  // stable callback identities and only re-render on data changes.
  const sidebarActionsRef = useRef<{
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
  } | null>(null);
  sidebarActionsRef.current = {
    connect: connectConnection,
    delete: setDeleteConfirm,
    deleteSavedSql,
    designTable,
    disconnect: disconnectConnection,
    duplicate: (connection) => duplicateMutation.mutate(connection),
    edit: handleEditConnection,
    newConnection: handleNewConnection,
    newQuery: (connection) => startNewQuery(connection?.id),
    openSavedSql,
    previewTable: (connectionId, table) =>
      browseTablePage(connectionId, table, 0, DEFAULT_PREVIEW_PAGE_SIZE),
    refresh: refreshConnectionsAndSchema,
    refreshSchema: refreshConnectionSchema,
    selectConnection: (connection) => selectConnection(connection.id),
    selectTable,
    toggleCatalog: loadCatalogSchema,
    toggleConnection: loadConnectionRoot,
    useSql: loadSqlIntoEditor,
  };

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
      onNewQuery: (connection?: DatabaseConnection) => sidebarActionsRef.current?.newQuery(connection),
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
    if (!onShellSidebarChange) {
      return;
    }
    onShellSidebarChange(shellSidebar);
    return () => onShellSidebarChange(null);
  }, [onShellSidebarChange, shellSidebar]);

  const toolbarConnectionId = activeQueryTab
    ? activeQueryTab.connectionId
    : activeTableTab?.connectionId ?? selectedConnectionId;
  const toolbarConnection = connections.find((item) => item.id === toolbarConnectionId) ?? null;
  const toolbarSession = toolbarConnectionId ? connectionStates[toolbarConnectionId] : undefined;
  const toolbarConnectionStatus: DatabaseConnectionStatus = toolbarSession?.status ?? "disconnected";
  const executePending = sqlRunning || browseMutation.isPending || rowMutation.isPending;
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
    if (!onShellStatusBarChange) {
      return;
    }
    onShellStatusBarChange(shellStatusBar);
    return () => onShellStatusBarChange(null);
  }, [onShellStatusBarChange, shellStatusBar]);

  // Inline editing is available when a real table with a primary key is being
  // browsed on a connected session; the primary key locates rows for the
  // update/delete row commands.
  const activeTableConnection = activeTableTab
    ? connections.find((connection) => connection.id === activeTableTab.connectionId) ?? null
    : null;
  const activeTableStatus = activeTableTab
    ? connectionStates[activeTableTab.connectionId]?.status ?? "disconnected"
    : "disconnected";
  const tableEditing: TableEditing | null = useMemo(
    () => createTableEditing({
      applyPendingChanges: applyPendingTableChanges,
      connection: activeTableConnection,
      connected: activeTableStatus === "connected",
      mutationPending: rowMutation.isPending,
      tab: activeTableTab,
      updateTableTab: databaseTabs.updateTableTab,
    }),
    [
      activeTableConnection,
      activeTableStatus,
      activeTableTab,
      applyPendingTableChanges,
      databaseTabs.updateTableTab,
      rowMutation.isPending,
    ],
  );

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      <DatabaseModuleToolbar
        connectionStatus={toolbarConnectionStatus}
        onNewQuery={startNewQuery}
        selectedConnectionName={toolbarConnection?.name ?? null}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        {selectedConnection && selectedConnectionStatus === "failed" ? (
          <DatabaseConnectionErrorBanner
            connectionName={selectedConnection.name}
            message={selectedSession?.message}
            onEdit={() => handleEditConnection(selectedConnection)}
            onRetry={() => connectConnection(selectedConnection)}
          />
        ) : null}
        <DatabaseWorkspace
          activeTab={activeTab}
          activeTabId={databaseTabs.activeTabId}
          connections={connections}
          executePending={executePending}
          history={queryHistory}
          catalogOptions={catalogOptions}
          onChangeQueryContext={changeQueryContext}
          onClearSql={clearSql}
          onCloseTab={databaseTabs.closeTab}
          onClearHistory={clearQueryHistory}
          onPreviewSelectedTable={previewSelectedTable}
          onRefreshSchema={refreshActiveSchema}
          onReorderTabs={databaseTabs.reorderTabs}
          onRun={runSql}
          onSelectConnection={(connectionId) => selectQueryConnection(connectionId || null)}
          onSelectResultSet={selectQueryResult}
          queryCatalog={activeQueryTab?.catalog ?? null}
          querySchema={activeQueryTab?.schema ?? null}
          schemaOptions={schemaOptions}
          onSelectHistory={loadHistoryEntry}
          onSelectResultTab={handleSelectResultTab}
          onSelectStructureTab={handleSelectStructureTab}
          onSelectTab={selectDatabaseTab}
          onSelectTableSegment={handleSelectTableSegment}
          onShowHistory={showQueryHistory}
          onSqlChange={updateActiveSql}
          onStop={stopQuery}
          onTableFilter={applyTableFilter}
          onTablePageChange={handleTablePageChange}
          onTableSort={applyTableSort}
          schema={visibleSchema}
          schemaError={schemaQuery.error}
          structure={structureQuery.data}
          structureError={structureQuery.error}
          structureLoading={structureEnabled && structureQuery.isFetching}
          tableEditing={tableEditing}
          tabs={databaseTabs.tabs}
          workspaceId={workspaceId}
        />
      </div>
      <DatabaseConnectionDialog
        canTest={canTest}
        error={saveMutation.error}
        form={form}
        onOpenChange={setEditorOpen}
        onPasswordChange={setPassword}
        onSubmit={submitConnection}
        onTest={testConnectionInput}
        onUpdate={updateForm}
        open={editorOpen}
        password={password}
        savePending={saveMutation.isPending}
        testPending={testInputMutation.isPending}
      >
        <DatabaseTestResultDialog
          onOpenChange={(open) => !open && setTestResult(null)}
          result={testResult}
        />
      </DatabaseConnectionDialog>
      <ConfirmDialog
        confirmLabel={t("common.actions.delete")}
        description={
          deleteConfirm ? t("database.tree.deleteBody", { name: deleteConfirm.name }) : ""
        }
        onConfirm={() => deleteConfirm && deleteMutation.mutate(deleteConfirm.id)}
        onOpenChange={(open) => !open && setDeleteConfirm(null)}
        open={deleteConfirm !== null}
        pending={deleteMutation.isPending}
        title={t("database.tree.deleteTitle")}
      />
    </div>
  );
}
