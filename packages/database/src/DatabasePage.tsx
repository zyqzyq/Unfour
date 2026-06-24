import { CheckCircle2, Save, Trash2, XCircle } from "lucide-react";
import { FormEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createCredential,
  deleteDatabaseConnection,
  mutateDatabaseRow,
  rotateCredential,
  saveDatabaseConnection,
  testDatabaseConnection,
} from "@unfour/command-client";
import type {
  DatabaseCellValue,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseTable,
  DatabaseTestResult,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import {
  Button,
  ConfirmDialog,
  Dialog,
  DialogBody,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  ErrorState,
  IconButton,
  Input,
  Select,
  StatusBadge,
  useI18n,
} from "@unfour/ui";
import { DatabaseSidebar } from "./components/DatabaseSidebar";
import { DatabaseErrorDetails } from "./components/DatabaseErrorDetails";
import { DatabaseModuleToolbar } from "./components/DatabaseModuleToolbar";
import { DatabaseStatusBar } from "./components/DatabaseStatusBar";
import { DatabaseWorkspace } from "./components/DatabaseWorkspace";
import { useDatabaseConnections } from "./hooks/useDatabaseConnections";
import { useDatabaseLayout } from "./hooks/useDatabaseLayout";
import { useQueryHistory } from "./hooks/useQueryHistory";
import { useSchemaTree } from "./hooks/useSchemaTree";
import { useSqlExecution } from "./hooks/useSqlExecution";
import { useTableData } from "./hooks/useTableData";
import { useTableStructure } from "./hooks/useTableStructure";
import { defaultSql } from "./model/database-state";
import { databaseTableTreeId } from "./model/database-tree";
import type {
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  DatabaseTableViewState,
  SqlHistoryEntry,
  TableEditing,
} from "./model/types";
import { describeDatabaseError, formatDatabaseError, isConfirmationRequired } from "./result-utils";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;
const MAX_HISTORY_ENTRIES = 25;

export function DatabasePage({
  onShellSidebarChange,
  workspaceId,
}: {
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const {
    selectedDatabaseConnectionId: selectedConnectionId,
    setSelectedDatabaseConnection,
  } = useWorkspaceStore();
  const layout = useDatabaseLayout();
  const [editorOpen, setEditorOpen] = useState(false);
  const [clientError, setClientError] = useState<unknown>(null);
  const [connectionStates, setConnectionStates] = useState<Record<string, DatabaseConnectionSessionState>>({});
  const [testResult, setTestResult] = useState<DatabaseTestResult | null>(null);
  const [queryHistory, setQueryHistory] = useState<SqlHistoryEntry[]>([]);
  const [queryResult, setQueryResult] = useState<DatabaseQueryResult | null>(null);
  const [pendingSqlConfirmation, setPendingSqlConfirmation] = useState(false);
  const [sql, setSql] = useState(defaultSql);
  const [tableView, setTableView] = useState<DatabaseTableViewState | null>(null);
  const [selectedTable, setSelectedTable] = useState<DatabaseTable | null>(null);
  const [password, setPassword] = useState("");
  const [form, setForm] = useState<DatabaseConnectionInput>({
    workspaceId,
    name: "Local SQLite",
    driver: "sqlite",
    sqlitePath: "",
  });

  const connectionsQuery = useDatabaseConnections(workspaceId);
  const queryHistoryQuery = useQueryHistory(workspaceId, MAX_HISTORY_ENTRIES);
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
  const prevSelectedConnectionIdRef = useRef(selectedConnectionId);
  // Tracks the SQL actually sent to the backend (may be a highlighted
  // selection rather than the full editor contents) so history reflects it.
  const executedSqlRef = useRef(sql);
  const selectedSession = selectedConnectionId ? connectionStates[selectedConnectionId] : undefined;
  const selectedConnectionStatus: DatabaseConnectionStatus = selectedSession?.status ?? "disconnected";
  const schemaEnabled = Boolean(
    selectedConnection &&
      (selectedConnectionStatus === "connecting" || selectedConnectionStatus === "connected"),
  );
  const schemaQuery = useSchemaTree({
    connection: selectedConnection,
    connectionId: selectedConnectionId,
    enabled: schemaEnabled,
    workspaceId,
  });
  const visibleSchema = schemaEnabled ? schemaQuery.data : undefined;
  const structureEnabled = Boolean(
    selectedConnection &&
      selectedTable &&
      layout.activeTabId === "table" &&
      layout.tableSegment === "structure" &&
      (selectedConnectionStatus === "connecting" || selectedConnectionStatus === "connected"),
  );
  const structureQuery = useTableStructure({
    connectionId: selectedConnectionId,
    enabled: structureEnabled,
    table: selectedTable,
    workspaceId,
  });
  const selectedTableId =
    selectedConnectionId && selectedTable ? databaseTableTreeId(selectedConnectionId, selectedTable) : null;

  useEffect(() => {
    if (selectedConnectionId && !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedDatabaseConnection(null);
      // eslint-disable-next-line react-hooks/set-state-in-effect -- clearing derived state when parent selection is removed
      setSelectedTable(null);
      setTableView(null);
    }

    setConnectionStates((current) => {
      const connectionIds = new Set(connections.map((connection) => connection.id));
      const next: Record<string, DatabaseConnectionSessionState> = {};
      for (const [connectionId, state] of Object.entries(current)) {
        if (connectionIds.has(connectionId)) {
          next[connectionId] = state;
        }
      }
      return next;
    });
  }, [connections, selectedConnectionId, setSelectedDatabaseConnection]);

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
        sqlitePath: selectedConnection.sqlitePath,
        credentialRef: selectedConnection.credentialRef,
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
      setConnectionStates((current) => {
        const next = { ...current };
        delete next[connectionId];
        return next;
      });
      // Only reset the active workspace when the deleted connection was the one
      // in use; deleting another connection from the context menu must not clear
      // the current query or table view.
      if (connectionId === selectedConnectionId) {
        setSelectedDatabaseConnection(null);
        setTestResult(null);
        setQueryResult(null);
        setTableView(null);
        setSelectedTable(null);
        setPendingSqlConfirmation(false);
        setClientError(null);
      }
      setDeleteConfirm(null);
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

  const executeMutation = useSqlExecution({
    connectionId: selectedConnectionId,
    onConfirmationRequired: setPendingSqlConfirmation,
    onError: (error) => {
      layout.setResultTab("results");
      if (isConfirmationRequired(error)) {
        return;
      }

      recordFailedHistory(error);
      const description = describeDatabaseError(error);
      if (selectedConnectionId && ["connection", "network", "permission"].includes(description.category)) {
        setConnectionState(selectedConnectionId, {
          message: description.message,
          status: "failed",
        });
      }
    },
    onExecuteStart: () => {
      setClientError(null);
      setTableView(null);
      layout.setActiveTabId("query");
      layout.setResultTab("results");
    },
    onSuccess: (result) => {
      setTableView(null);
      setQueryResult(result);
      layout.setResultTab("results");
      if (selectedConnectionId) {
        setConnectionState(selectedConnectionId, {
          message: t("database.query.completed", {
            durationMs: result.durationMs,
          }),
          status: "connected",
        });
      }
      recordSuccessfulHistory(result);
    },
    workspaceId,
  });

  const browseMutation = useTableData({
    connectionId: selectedConnectionId,
    onBrowseStart: () => {
      setClientError(null);
      setPendingSqlConfirmation(false);
      layout.setActiveTabId("table");
      layout.setTableSegment("data");
      layout.setResultTab("results");
    },
    onSuccess: (browse) => {
      setPendingSqlConfirmation(false);
      setQueryResult(browse.result);
      setTableView({
        pageIndex: Math.floor(browse.offset / Math.max(1, browse.limit)),
        pageSize: browse.limit,
        readOnly: browse.readOnly,
        tableName: browse.tableName,
        totalRows: browse.totalRows,
      });
      if (selectedConnectionId) {
        setConnectionState(selectedConnectionId, {
          message: t("database.query.previewLoaded", {
            count: browse.result.rows.length,
          }),
          status: "connected",
        });
      }
    },
    workspaceId,
  });

  const rowMutation = useMutation({
    mutationFn: mutateDatabaseRow,
    onSuccess: () => {
      // Re-read the current page so the grid reflects the committed change.
      refreshTablePage();
    },
    onError: (error) => {
      layout.setResultTab("results");
      setClientError(error);
    },
  });

  useEffect(() => {
    if (!selectedConnectionId || !browseMutation.error) {
      return;
    }

    const description = describeDatabaseError(browseMutation.error);
    if (["connection", "network", "permission"].includes(description.category)) {
      setConnectionState(selectedConnectionId, {
        message: description.message,
        status: "failed",
      });
    }
  }, [browseMutation.error, selectedConnectionId]);

  function setConnectionState(connectionId: string, patch: Partial<DatabaseConnectionSessionState>) {
    setConnectionStates((current) => ({
      ...current,
      [connectionId]: {
        message: patch.message ?? current[connectionId]?.message ?? null,
        serverVersion: patch.serverVersion ?? current[connectionId]?.serverVersion ?? null,
        status: patch.status ?? current[connectionId]?.status ?? "disconnected",
        updatedAt: new Date().toISOString(),
      },
    }));
  }

  function updateForm(patch: Partial<DatabaseConnectionInput>) {
    setForm((current) => ({ ...current, ...patch, workspaceId }));
  }

  function submitConnection(event: FormEvent) {
    event.preventDefault();
    saveMutation.mutate({
      input: {
        ...form,
        workspaceId,
        credentialRef: form.credentialRef?.trim() || null,
        sqlitePath: form.sqlitePath?.trim() || null,
        host: form.host?.trim() || null,
        database: form.database?.trim() || null,
        username: form.username?.trim() || null,
      },
      secret: password,
    });
  }

  function selectConnection(connectionId: string | null) {
    setSelectedDatabaseConnection(connectionId);
    setClientError(null);
    setPendingSqlConfirmation(false);
    setTestResult(null);
    setSelectedTable(null);
    setTableView(null);
    setQueryResult(null);
  }

  function connectConnection(connection: DatabaseConnection) {
    selectConnection(connection.id);
    setSelectedTable(null);
    setTableView(null);
    testMutation.mutate(connection.id);
  }

  function connectSelectedConnection() {
    if (selectedConnection) {
      connectConnection(selectedConnection);
    }
  }

  function disconnectConnection(connection: DatabaseConnection) {
    setConnectionState(connection.id, {
      message: t("database.connection.disconnected"),
      status: "disconnected",
    });
    if (connection.id === selectedConnectionId) {
      setSelectedTable(null);
      setTableView(null);
      setQueryResult(null);
      setPendingSqlConfirmation(false);
      setClientError(null);
    }
  }

  function newConnection() {
    selectConnection(null);
    setPassword("");
    setForm({ workspaceId, name: "Local SQLite", driver: "sqlite", sqlitePath: "" });
  }

  function refreshConnectionsAndSchema() {
    queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    if (selectedConnectionId && selectedConnectionStatus !== "disconnected") {
      queryClient.invalidateQueries({ queryKey: ["database-schema", workspaceId, selectedConnectionId] });
    }
  }

  function refreshConnectionSchema(connection: DatabaseConnection) {
    const status = connectionStates[connection.id]?.status ?? "disconnected";
    if (connection.id !== selectedConnectionId) {
      selectConnection(connection.id);
    }

    if (status === "disconnected") {
      setClientError({
        code: "VALIDATION_ERROR",
        message: t("database.connection.connectBeforeRefresh"),
      });
      layout.setResultTab("results");
      return;
    }

    queryClient.invalidateQueries({ queryKey: ["database-schema", workspaceId, connection.id] });
  }

  function refreshSelectedSchema() {
    if (!selectedConnection) {
      return;
    }
    refreshConnectionSchema(selectedConnection);
  }

  function selectTable(table: DatabaseTable) {
    setSelectedTable(table);
    setClientError(null);
    layout.setActiveTabId("table");
    layout.setTableSegment("structure");
  }

  function browseTablePage(table: DatabaseTable, pageIndex: number, pageSize: number) {
    if (!selectedConnectionId) {
      setClientError({
        code: "VALIDATION_ERROR",
        message: t("database.errors.selectBeforePreview"),
      });
      layout.setResultTab("results");
      return;
    }

    setSelectedTable(table);
    setClientError(null);
    executeMutation.reset();
    browseMutation.reset();
    browseMutation.mutate({
      pageIndex: Math.max(0, pageIndex),
      pageSize,
      schema: table.schema,
      tableName: table.name,
    });
  }

  function previewSelectedTable() {
    if (!selectedTable) {
      return;
    }
    browseTablePage(selectedTable, 0, tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE);
  }

  function refreshTablePage() {
    if (selectedTable && tableView) {
      browseTablePage(selectedTable, tableView.pageIndex, tableView.pageSize);
    }
  }

  function mutateRow(
    operation: "insert" | "update" | "delete",
    values: DatabaseCellValue[],
    primaryKey: DatabaseCellValue[],
  ) {
    if (!selectedConnectionId || !selectedTable) {
      return;
    }
    setClientError(null);
    rowMutation.mutate({
      workspaceId,
      connectionId: selectedConnectionId,
      schema: selectedTable.schema,
      tableName: selectedTable.name,
      operation,
      values,
      primaryKey,
    });
  }

  function runSql(overrideSql?: string) {
    executeMutation.reset();
    browseMutation.reset();
    setClientError(null);

    if (!selectedConnectionId) {
      setQueryResult(null);
      setClientError({
        code: "VALIDATION_ERROR",
        message: t("database.errors.selectBeforeRun"),
      });
      layout.setResultTab("results");
      return;
    }

    // Run the highlighted statement when the editor reports a non-empty
    // selection; otherwise fall back to the full editor contents.
    const effectiveSql = overrideSql && overrideSql.trim() ? overrideSql : sql;
    if (!effectiveSql.trim()) {
      setQueryResult(null);
      setClientError({
        code: "VALIDATION_ERROR",
        message: t("database.errors.sqlEmpty"),
      });
      layout.setResultTab("results");
      return;
    }

    executedSqlRef.current = effectiveSql;
    executeMutation.mutate({ confirmMutation: pendingSqlConfirmation, sql: effectiveSql });
  }

  function clearSql() {
    setSql("");
    setClientError(null);
    setPendingSqlConfirmation(false);
    executeMutation.reset();
  }

  function startNewQuery() {
    clearSql();
    setQueryResult(null);
    setTableView(null);
    layout.setActiveTabId("query");
    layout.setResultTab("results");
  }

  function showQueryHistory() {
    layout.setActiveTabId("query");
    layout.setResultTab("history");
  }

  function recordSuccessfulHistory(result: DatabaseQueryResult) {
    appendHistory({
      affectedRows: result.affectedRows,
      classification: result.safety.classification,
      connectionId: selectedConnectionId,
      connectionName: selectedConnection?.name ?? t("database.query.unknownConnection"),
      durationMs: result.durationMs,
      rowCount: result.rows.length,
      sql: executedSqlRef.current,
      status: "success",
    });
  }

  function recordFailedHistory(error: unknown) {
    appendHistory({
      connectionId: selectedConnectionId,
      connectionName: selectedConnection?.name ?? t("database.query.unknownConnection"),
      error: formatDatabaseError(error),
      sql: executedSqlRef.current,
      status: "failed",
    });
  }

  function appendHistory(entry: Omit<SqlHistoryEntry, "executedAt" | "id">) {
    const now = new Date().toISOString();
    const historyEntry: SqlHistoryEntry = {
      ...entry,
      executedAt: now,
      id: `${now}-${Math.random().toString(36).slice(2, 8)}`,
    };
    setQueryHistory((current) => [historyEntry, ...current].slice(0, MAX_HISTORY_ENTRIES));
    queryHistoryQuery.record(historyEntry);
  }

  function clearQueryHistory() {
    setQueryHistory([]);
    queryHistoryQuery.clear();
  }

  function loadHistoryEntry(entry: SqlHistoryEntry) {
    setSql(entry.sql);
    if (entry.connectionId && connections.some((connection) => connection.id === entry.connectionId)) {
      setSelectedDatabaseConnection(entry.connectionId);
    }
    layout.setActiveTabId("query");
  }

  // Load generated SQL (e.g. from a table context-menu action) into the editor.
  function loadSqlIntoEditor(generatedSql: string) {
    setSql(generatedSql);
    setClientError(null);
    setPendingSqlConfirmation(false);
    layout.setActiveTabId("query");
    layout.setResultTab("results");
  }

  function handleNewConnection() {
    newConnection();
    setEditorOpen(true);
  }

  function handleEditConnection(connection: DatabaseConnection) {
    selectConnection(connection.id);
    setEditorOpen(true);
  }

  // Keep the latest handlers in a ref (render-time write, matching the existing
  // prevSelectedConnectionIdRef pattern) so the pushed shell sidebar can use
  // stable callback identities and only re-render on data changes.
  const sidebarActionsRef = useRef<{
    connect: (connection: DatabaseConnection) => void;
    delete: (connection: DatabaseConnection) => void;
    disconnect: (connection: DatabaseConnection) => void;
    edit: (connection: DatabaseConnection) => void;
    newConnection: () => void;
    newQuery: () => void;
    previewTable: (table: DatabaseTable) => void;
    refresh: () => void;
    refreshSchema: (connection: DatabaseConnection) => void;
    selectConnection: (connection: DatabaseConnection) => void;
    selectTable: (table: DatabaseTable) => void;
    useSql: (sql: string) => void;
  } | null>(null);
  sidebarActionsRef.current = {
    connect: connectConnection,
    delete: setDeleteConfirm,
    disconnect: disconnectConnection,
    edit: handleEditConnection,
    newConnection: handleNewConnection,
    newQuery: startNewQuery,
    previewTable: (table) => browseTablePage(table, 0, tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE),
    refresh: refreshConnectionsAndSchema,
    refreshSchema: refreshConnectionSchema,
    selectConnection: (connection) => selectConnection(connection.id),
    selectTable,
    useSql: loadSqlIntoEditor,
  };

  const sidebarHandlers = useMemo(
    () => ({
      onConnect: (connection: DatabaseConnection) => sidebarActionsRef.current?.connect(connection),
      onDeleteConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.delete(connection),
      onDisconnect: (connection: DatabaseConnection) => sidebarActionsRef.current?.disconnect(connection),
      onEditConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.edit(connection),
      onNewConnection: () => sidebarActionsRef.current?.newConnection(),
      onNewQuery: () => sidebarActionsRef.current?.newQuery(),
      onPreviewTable: (table: DatabaseTable) => sidebarActionsRef.current?.previewTable(table),
      onRefresh: () => sidebarActionsRef.current?.refresh(),
      onRefreshSchema: (connection: DatabaseConnection) => sidebarActionsRef.current?.refreshSchema(connection),
      onSelectConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.selectConnection(connection),
      onSelectTable: (table: DatabaseTable) => sidebarActionsRef.current?.selectTable(table),
      onUseSql: (sql: string) => sidebarActionsRef.current?.useSql(sql),
    }),
    [],
  );

  const schemaLoadingFlag = schemaEnabled && schemaQuery.isFetching;
  const shellSidebar = useMemo(
    () => (
      <DatabaseSidebar
        connectionStates={connectionStates}
        connections={connections}
        schema={visibleSchema}
        schemaLoading={schemaLoadingFlag}
        selectedConnectionId={selectedConnectionId}
        selectedTableId={selectedTableId}
        {...sidebarHandlers}
      />
    ),
    [
      connectionStates,
      connections,
      schemaLoadingFlag,
      selectedConnectionId,
      selectedTableId,
      sidebarHandlers,
      visibleSchema,
    ],
  );

  useEffect(() => {
    if (!onShellSidebarChange) {
      return;
    }
    onShellSidebarChange(shellSidebar);
    return () => onShellSidebarChange(null);
  }, [onShellSidebarChange, shellSidebar]);

  const activeError = clientError ?? (layout.activeTabId === "table" ? browseMutation.error : executeMutation.error);
  const executePending = executeMutation.isPending || browseMutation.isPending;

  // Inline editing is available when a real table with a primary key is being
  // browsed on a connected session; the primary key locates rows for the
  // update/delete row commands.
  const primaryKeyColumns = (selectedTable?.columns ?? [])
    .filter((column) => column.primaryKey)
    .map((column) => column.name);
  const tableEditing: TableEditing | null =
    selectedTable && tableView && selectedConnectionStatus === "connected" && primaryKeyColumns.length > 0
      ? {
          pending: rowMutation.isPending,
          primaryKeyColumns,
          onDeleteRow: (primaryKey) => mutateRow("delete", [], primaryKey),
          onInsertRow: (values) => mutateRow("insert", values, []),
          onUpdateCell: (columnName, value, primaryKey) =>
            mutateRow("update", [{ column: columnName, value }], primaryKey),
        }
      : null;

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-surface)]">
      <DatabaseModuleToolbar
        connectionStatus={selectedConnectionStatus}
        connections={connections}
        executePending={executePending}
        onClearSql={clearSql}
        onConnect={connectSelectedConnection}
        onDisconnect={() => selectedConnection && disconnectConnection(selectedConnection)}
        onNewQuery={startNewQuery}
        onRefresh={refreshConnectionsAndSchema}
        onRun={runSql}
        onSelectConnection={(connectionId) => selectConnection(connectionId || null)}
        onStop={() => undefined}
        pendingConfirmation={pendingSqlConfirmation}
        selectedConnectionId={selectedConnectionId}
        sqlDirty={sql.trim().length > 0}
      />
      <div className="min-h-0 flex-1">
        <DatabaseWorkspace
          activeResultTab={layout.resultTab}
          activeStructureTab={layout.inspectorTab}
          activeTabId={layout.activeTabId}
          connections={connections}
          error={activeError}
          executePending={executePending}
          history={queryHistory}
          onClearSql={clearSql}
          onClearHistory={clearQueryHistory}
          onPreviewSelectedTable={previewSelectedTable}
          onRefreshSchema={refreshSelectedSchema}
          onRun={runSql}
          onSelectConnection={(connectionId) => selectConnection(connectionId || null)}
          onSelectHistory={loadHistoryEntry}
          onSelectResultTab={layout.setResultTab}
          onSelectStructureTab={layout.setInspectorTab}
          onSelectTab={layout.setActiveTabId}
          onSelectTableSegment={layout.setTableSegment}
          onShowHistory={showQueryHistory}
          onSqlChange={setSql}
          onStop={() => undefined}
          onTablePageChange={(pageIndex, pageSize) => selectedTable && browseTablePage(selectedTable, pageIndex, pageSize)}
          pendingConfirmation={pendingSqlConfirmation}
          queryResult={queryResult}
          schema={visibleSchema}
          schemaError={schemaQuery.error}
          selectedConnectionId={selectedConnectionId}
          selectedTable={selectedTable}
          sql={sql}
          structure={structureQuery.data}
          structureError={structureQuery.error}
          structureLoading={structureEnabled && structureQuery.isFetching}
          tableEditing={tableEditing}
          tableSegment={layout.tableSegment}
          tableView={tableView}
        />
      </div>
      <DatabaseStatusBar connection={selectedConnection} executing={executePending} session={selectedSession} />
      <DatabaseConnectionDialog
        error={saveMutation.error ?? testMutation.error}
        form={form}
        onDelete={() => {
          const target = connections.find((item) => item.id === selectedConnectionId);
          if (target) {
            setDeleteConfirm(target);
          }
        }}
        onOpenChange={setEditorOpen}
        onPasswordChange={setPassword}
        onSubmit={submitConnection}
        onTest={connectSelectedConnection}
        onUpdate={updateForm}
        open={editorOpen}
        password={password}
        result={testResult}
        savePending={saveMutation.isPending}
        selectedConnectionId={selectedConnectionId}
        testPending={testMutation.isPending}
      />
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

function DatabaseConnectionDialog({
  error,
  form,
  onDelete,
  onOpenChange,
  onPasswordChange,
  onSubmit,
  onTest,
  onUpdate,
  open,
  password,
  result,
  savePending,
  selectedConnectionId,
  testPending,
}: {
  error: unknown;
  form: DatabaseConnectionInput;
  onDelete: () => void;
  onOpenChange: (open: boolean) => void;
  onPasswordChange: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
  onTest: () => void;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void;
  open: boolean;
  password: string;
  result: DatabaseTestResult | null;
  savePending: boolean;
  selectedConnectionId: string | null;
  testPending: boolean;
}) {
  const { t } = useI18n();

  return (
    <Dialog onOpenChange={onOpenChange} open={open}>
      <DialogContent title={t("database.connection.settings")}>
        <DialogHeader>
          <DialogTitle>{t("database.connection.settings")}</DialogTitle>
        </DialogHeader>
        <form onSubmit={onSubmit}>
          <DialogBody className="space-y-2">
            <Field title={t("database.fields.name")}>
              <Input onChange={(event) => onUpdate({ name: event.target.value })} value={form.name} />
            </Field>
            <Field title={t("database.fields.driver")}>
              <Select
                onChange={(event) =>
                  onUpdate({
                    driver: event.target.value as DatabaseConnectionInput["driver"],
                    sqlitePath: event.target.value === "sqlite" ? form.sqlitePath : null,
                    credentialRef: event.target.value === "sqlite" ? null : form.credentialRef,
                  })
                }
                options={[
                  { label: t("database.driver.sqlite"), value: "sqlite" },
                  { label: t("database.driver.postgres"), value: "postgres" },
                  { label: t("database.driver.mysql"), value: "mysql" },
                ]}
                value={form.driver}
              />
            </Field>
            {form.driver === "sqlite" ? (
              <Field title={t("database.fields.sqlitePath")}>
                <Input onChange={(event) => onUpdate({ sqlitePath: event.target.value })} placeholder="E:\\data\\app.sqlite" value={form.sqlitePath ?? ""} />
              </Field>
            ) : (
              <>
                <div className="grid grid-cols-[1fr_76px] gap-2">
                  <Field title={t("database.fields.host")}>
                    <Input onChange={(event) => onUpdate({ host: event.target.value })} placeholder="127.0.0.1" value={form.host ?? ""} />
                  </Field>
                  <Field title={t("database.fields.port")}>
                    <Input
                      onChange={(event) => onUpdate({ port: event.target.value ? Number(event.target.value) : null })}
                      placeholder={form.driver === "postgres" ? "5432" : "3306"}
                      type="number"
                      value={form.port ?? ""}
                    />
                  </Field>
                </div>
                <Field title={t("database.fields.database")}>
                  <Input onChange={(event) => onUpdate({ database: event.target.value })} value={form.database ?? ""} />
                </Field>
                <Field title={t("database.fields.username")}>
                  <Input onChange={(event) => onUpdate({ username: event.target.value })} value={form.username ?? ""} />
                </Field>
                <Field title={t("database.fields.password")}>
                  <Input
                    autoComplete="off"
                    onChange={(event) => onPasswordChange(event.target.value)}
                    placeholder={form.credentialRef ? t("database.fields.passwordKeep") : ""}
                    type="password"
                    value={password}
                  />
                </Field>
              </>
            )}
            {error ? (
              <ErrorState className="min-h-[48px]">
                <DatabaseErrorDetails error={error} />
              </ErrorState>
            ) : null}
            {result && (
              <div className="flex items-center gap-2 text-[12px] text-[var(--u-color-text-muted)]">
                {result.ok ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
                <span className="min-w-0 flex-1 truncate">{String(result.message)}</span>
                <StatusBadge tone={result.ok ? "success" : "warning"}>
                  {result.ok ? t("database.connection.connected") : t("database.connection.failed")}
                </StatusBadge>
              </div>
            )}
          </DialogBody>
          <DialogFooter className="justify-between">
            <IconButton disabled={!selectedConnectionId} label={t("database.connection.deleteLabel", "Delete database connection")} onClick={onDelete}>
              <Trash2 size={13} />
            </IconButton>
            <div className="flex items-center gap-2">
              <Button onClick={() => onOpenChange(false)} size="sm" type="button" variant="ghost">
                {t("common.confirm.cancel")}
              </Button>
              <Button disabled={!selectedConnectionId || testPending} onClick={onTest} size="sm" type="button" variant="outline">
                <CheckCircle2 size={13} />
                {testPending ? t("common.actions.connecting") : t("common.actions.connect")}
              </Button>
              <Button disabled={savePending} size="sm" type="submit">
                <Save size={13} />
                {t("common.actions.save")}
              </Button>
            </div>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function Field({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <label className="block space-y-1">
      <span className="text-[11px] font-medium uppercase text-[var(--u-color-text-soft)]">{title}</span>
      {children}
    </label>
  );
}
