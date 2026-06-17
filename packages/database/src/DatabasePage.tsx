import { CheckCircle2, Database, Plus, RefreshCw, Save, Table2, Trash2, XCircle } from "lucide-react";
import { FormEvent, type ReactNode, useEffect, useMemo, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  deleteDatabaseConnection,
  saveDatabaseConnection,
  testDatabaseConnection,
} from "@unfour/command-client";
import type {
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseTable,
  DatabaseTestResult,
} from "@unfour/command-client";
import { useWorkspaceStore } from "@unfour/workspace-core";
import {
  Badge,
  Button,
  ErrorState,
  IconButton,
  Input,
  Select,
  StatusBadge,
  Toolbar,
  ToolbarGroup,
} from "@unfour/ui";
import { DatabaseConnectionTree } from "./components/DatabaseConnectionTree";
import { DatabaseErrorDetails } from "./components/DatabaseErrorDetails";
import { DatabaseModuleToolbar } from "./components/DatabaseModuleToolbar";
import { DatabaseStatusBar } from "./components/DatabaseStatusBar";
import { DatabaseWorkspace } from "./components/DatabaseWorkspace";
import { useDatabaseConnections } from "./hooks/useDatabaseConnections";
import { useDatabaseLayout } from "./hooks/useDatabaseLayout";
import { useSchemaTree } from "./hooks/useSchemaTree";
import { useSqlExecution } from "./hooks/useSqlExecution";
import { useTableData } from "./hooks/useTableData";
import { defaultSql } from "./model/database-state";
import { databaseTableTreeId } from "./model/database-tree";
import type {
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  DatabaseTableViewState,
  SqlHistoryEntry,
} from "./model/types";
import { describeDatabaseError, formatDatabaseError, isConfirmationRequired } from "./result-utils";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;
const MAX_HISTORY_ENTRIES = 25;

export function DatabasePage({ workspaceId }: { workspaceId: string }) {
  const queryClient = useQueryClient();
  const {
    selectedDatabaseConnectionId: selectedConnectionId,
    setSelectedDatabaseConnection,
  } = useWorkspaceStore();
  const layout = useDatabaseLayout();
  const [clientError, setClientError] = useState<unknown>(null);
  const [connectionStates, setConnectionStates] = useState<Record<string, DatabaseConnectionSessionState>>({});
  const [testResult, setTestResult] = useState<DatabaseTestResult | null>(null);
  const [queryHistory, setQueryHistory] = useState<SqlHistoryEntry[]>([]);
  const [queryResult, setQueryResult] = useState<DatabaseQueryResult | null>(null);
  const [pendingSqlConfirmation, setPendingSqlConfirmation] = useState(false);
  const [sql, setSql] = useState(defaultSql);
  const [tableView, setTableView] = useState<DatabaseTableViewState | null>(null);
  const [selectedTable, setSelectedTable] = useState<DatabaseTable | null>(null);
  const [form, setForm] = useState<DatabaseConnectionInput>({
    workspaceId,
    name: "Local SQLite",
    driver: "sqlite",
    sqlitePath: "",
  });

  const connectionsQuery = useDatabaseConnections(workspaceId);
  const connections = useMemo(() => connectionsQuery.data ?? [], [connectionsQuery.data]);
  const selectedConnection = useMemo(
    () => connections.find((item) => item.id === selectedConnectionId) ?? null,
    [connections, selectedConnectionId],
  );
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
  const selectedTableId =
    selectedConnectionId && selectedTable ? databaseTableTreeId(selectedConnectionId, selectedTable) : null;

  useEffect(() => {
    if (selectedConnectionId && !connections.some((connection) => connection.id === selectedConnectionId)) {
      setSelectedDatabaseConnection(null);
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
    setForm((current) => ({ ...current, workspaceId }));
  }, [workspaceId]);

  useEffect(() => {
    if (!selectedConnection) {
      return;
    }

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
  }, [selectedConnection, workspaceId]);

  useEffect(() => {
    if (!selectedConnectionId || !schemaEnabled || !schemaQuery.data) {
      return;
    }

    setConnectionState(selectedConnectionId, {
      message: `${schemaQuery.data.tables.length} tables loaded`,
      status: "connected",
    });
  }, [schemaEnabled, schemaQuery.data, selectedConnectionId]);

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
    mutationFn: saveDatabaseConnection,
    onSuccess: (connection) => {
      setSelectedDatabaseConnection(connection.id);
      setConnectionState(connection.id, {
        message: "Saved. Connect to browse schema.",
        status: "disconnected",
      });
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const deleteMutation = useMutation({
    mutationFn: (connectionId: string) => deleteDatabaseConnection(workspaceId, connectionId),
    onSuccess: () => {
      if (selectedConnectionId) {
        setConnectionStates((current) => {
          const next = { ...current };
          delete next[selectedConnectionId];
          return next;
        });
      }
      setSelectedDatabaseConnection(null);
      setTestResult(null);
      setQueryResult(null);
      setTableView(null);
      setSelectedTable(null);
      setPendingSqlConfirmation(false);
      setClientError(null);
      queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    },
  });

  const testMutation = useMutation({
    mutationFn: (connectionId: string) => testDatabaseConnection(workspaceId, connectionId),
    onMutate: (connectionId) => {
      setConnectionState(connectionId, {
        message: "Connecting...",
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
      layout.setActiveTabId("sql-editor");
      layout.setResultTab("results");
    },
    onSuccess: (result) => {
      setTableView(null);
      setQueryResult(result);
      layout.setResultTab("results");
      if (selectedConnectionId) {
        setConnectionState(selectedConnectionId, {
          message: `Query completed in ${result.durationMs}ms`,
          status: "connected",
        });
      }
      recordSuccessfulHistory(result);
    },
    sql,
    workspaceId,
  });

  const browseMutation = useTableData({
    connectionId: selectedConnectionId,
    onBrowseStart: () => {
      setClientError(null);
      setPendingSqlConfirmation(false);
      layout.setActiveTabId("table-data");
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
          message: `Preview loaded: ${browse.result.rows.length} rows`,
          status: "connected",
        });
      }
    },
    workspaceId,
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
      ...form,
      credentialRef: form.credentialRef?.trim() || null,
      sqlitePath: form.sqlitePath?.trim() || null,
      host: form.host?.trim() || null,
      database: form.database?.trim() || null,
      username: form.username?.trim() || null,
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
      message: "Disconnected",
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
        message: "Connect to the database before refreshing schema.",
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
    layout.setActiveTabId("table-structure");
  }

  function browseTablePage(table: DatabaseTable, pageIndex: number, pageSize: number) {
    if (!selectedConnectionId) {
      setClientError({
        code: "VALIDATION_ERROR",
        message: "Select a database connection before opening table preview.",
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

  function runSql() {
    executeMutation.reset();
    browseMutation.reset();
    setClientError(null);

    if (!selectedConnectionId) {
      setQueryResult(null);
      setClientError({
        code: "VALIDATION_ERROR",
        message: "Select a database connection before running SQL.",
      });
      layout.setResultTab("results");
      return;
    }

    if (!sql.trim()) {
      setQueryResult(null);
      setClientError({
        code: "VALIDATION_ERROR",
        message: "SQL cannot be empty.",
      });
      layout.setResultTab("results");
      return;
    }

    executeMutation.mutate(pendingSqlConfirmation);
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
    layout.setActiveTabId("sql-editor");
    layout.setResultTab("results");
  }

  function recordSuccessfulHistory(result: DatabaseQueryResult) {
    appendHistory({
      affectedRows: result.affectedRows,
      classification: result.safety.classification,
      connectionId: selectedConnectionId,
      connectionName: selectedConnection?.name ?? "Unknown connection",
      durationMs: result.durationMs,
      rowCount: result.rows.length,
      sql,
      status: "success",
    });
  }

  function recordFailedHistory(error: unknown) {
    appendHistory({
      connectionId: selectedConnectionId,
      connectionName: selectedConnection?.name ?? "Unknown connection",
      error: formatDatabaseError(error),
      sql,
      status: "failed",
    });
  }

  function appendHistory(entry: Omit<SqlHistoryEntry, "executedAt" | "id">) {
    const now = new Date().toISOString();
    setQueryHistory((current) => [
      {
        ...entry,
        executedAt: now,
        id: `${now}-${Math.random().toString(36).slice(2, 8)}`,
      },
      ...current,
    ].slice(0, MAX_HISTORY_ENTRIES));
  }

  function loadHistoryEntry(entry: SqlHistoryEntry) {
    setSql(entry.sql);
    if (entry.connectionId && connections.some((connection) => connection.id === entry.connectionId)) {
      setSelectedDatabaseConnection(entry.connectionId);
    }
    layout.setActiveTabId("sql-editor");
  }

  const activeError = clientError ?? (layout.activeTabId === "table-data" ? browseMutation.error : executeMutation.error);
  const executePending = executeMutation.isPending || browseMutation.isPending;

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
      <div className="grid min-h-0 flex-1 grid-cols-[300px_minmax(0,1fr)]">
        <aside className="flex min-h-0 flex-col border-r border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]">
          <Toolbar className="h-8">
            <ToolbarGroup>
              <Database size={14} />
              <span className="text-[12px] font-semibold text-[var(--u-color-text)]">Connections</span>
              <Badge tone="neutral">{connections.length}</Badge>
            </ToolbarGroup>
            <ToolbarGroup>
              <IconButton label="New database connection" onClick={newConnection}>
                <Plus size={13} />
              </IconButton>
              <IconButton label="Refresh database connections" onClick={refreshConnectionsAndSchema}>
                <RefreshCw size={13} />
              </IconButton>
            </ToolbarGroup>
          </Toolbar>
          <div className="min-h-0 flex-1 overflow-auto p-2">
            <DatabaseConnectionTree
              connectionStates={connectionStates}
              connections={connections}
              onConnect={connectConnection}
              onDisconnect={disconnectConnection}
              onNewQuery={startNewQuery}
              onPreviewTable={(table) => browseTablePage(table, 0, tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE)}
              onRefresh={refreshConnectionsAndSchema}
              onRefreshSchema={refreshConnectionSchema}
              onSelectConnection={(connection) => selectConnection(connection.id)}
              onSelectTable={selectTable}
              schema={visibleSchema}
              schemaLoading={schemaEnabled && schemaQuery.isFetching}
              selectedConnectionId={selectedConnectionId}
              selectedTableId={selectedTableId}
            />
          </div>
          <ConnectionEditor
            error={saveMutation.error ?? testMutation.error}
            form={form}
            onDelete={() => selectedConnectionId && deleteMutation.mutate(selectedConnectionId)}
            onNew={newConnection}
            onSubmit={submitConnection}
            onTest={connectSelectedConnection}
            onUpdate={updateForm}
            result={testResult}
            savePending={saveMutation.isPending}
            selectedConnectionId={selectedConnectionId}
            testPending={testMutation.isPending}
          />
        </aside>
        <DatabaseWorkspace
          activeResultTab={layout.resultTab}
          activeStructureTab={layout.inspectorTab}
          activeTabId={layout.activeTabId}
          connections={connections}
          error={activeError}
          executePending={executePending}
          history={queryHistory}
          onClearSql={clearSql}
          onPreviewSelectedTable={previewSelectedTable}
          onRefreshSchema={refreshSelectedSchema}
          onRun={runSql}
          onSelectConnection={(connectionId) => selectConnection(connectionId || null)}
          onSelectHistory={loadHistoryEntry}
          onSelectResultTab={layout.setResultTab}
          onSelectStructureTab={layout.setInspectorTab}
          onSelectTab={layout.setActiveTabId}
          onSqlChange={setSql}
          onStop={() => undefined}
          onTablePageChange={(pageIndex, pageSize) => selectedTable && browseTablePage(selectedTable, pageIndex, pageSize)}
          pendingConfirmation={pendingSqlConfirmation}
          queryResult={queryResult}
          schemaError={schemaQuery.error}
          schemaLoading={schemaEnabled && schemaQuery.isFetching}
          selectedConnectionId={selectedConnectionId}
          selectedTable={selectedTable}
          sql={sql}
          tableView={tableView}
        />
      </div>
      <DatabaseStatusBar connection={selectedConnection} executing={executePending} session={selectedSession} />
    </div>
  );
}

function ConnectionEditor({
  error,
  form,
  onDelete,
  onNew,
  onSubmit,
  onTest,
  onUpdate,
  result,
  savePending,
  selectedConnectionId,
  testPending,
}: {
  error: unknown;
  form: DatabaseConnectionInput;
  onDelete: () => void;
  onNew: () => void;
  onSubmit: (event: FormEvent) => void;
  onTest: () => void;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void;
  result: DatabaseTestResult | null;
  savePending: boolean;
  selectedConnectionId: string | null;
  testPending: boolean;
}) {
  return (
    <form className="max-h-[46%] shrink-0 space-y-2 overflow-auto border-t border-[var(--u-color-border)] p-2" onSubmit={onSubmit}>
      <Toolbar className="h-8 border border-[var(--u-color-border)]">
        <ToolbarGroup>
          <Table2 size={14} />
          <span className="text-[12px] font-semibold text-[var(--u-color-text)]">Connection Settings</span>
        </ToolbarGroup>
        <ToolbarGroup>
          <IconButton label="New database connection" onClick={onNew}>
            <Plus size={13} />
          </IconButton>
        </ToolbarGroup>
      </Toolbar>
      <Field title="Name">
        <Input onChange={(event) => onUpdate({ name: event.target.value })} value={form.name} />
      </Field>
      <Field title="Driver">
        <Select
          onChange={(event) =>
            onUpdate({
              driver: event.target.value as DatabaseConnectionInput["driver"],
              sqlitePath: event.target.value === "sqlite" ? form.sqlitePath : null,
              credentialRef: event.target.value === "sqlite" ? null : form.credentialRef,
            })
          }
          options={[
            { label: "SQLite", value: "sqlite" },
            { label: "PostgreSQL", value: "postgres" },
            { label: "MySQL / MariaDB", value: "mysql" },
          ]}
          value={form.driver}
        />
      </Field>
      {form.driver === "sqlite" ? (
        <Field title="SQLite Path">
          <Input onChange={(event) => onUpdate({ sqlitePath: event.target.value })} placeholder="E:\\data\\app.sqlite" value={form.sqlitePath ?? ""} />
        </Field>
      ) : (
        <>
          <div className="grid grid-cols-[1fr_76px] gap-2">
            <Field title="Host">
              <Input onChange={(event) => onUpdate({ host: event.target.value })} placeholder="127.0.0.1" value={form.host ?? ""} />
            </Field>
            <Field title="Port">
              <Input
                onChange={(event) => onUpdate({ port: event.target.value ? Number(event.target.value) : null })}
                placeholder={form.driver === "postgres" ? "5432" : "3306"}
                type="number"
                value={form.port ?? ""}
              />
            </Field>
          </div>
          <Field title="Database">
            <Input onChange={(event) => onUpdate({ database: event.target.value })} value={form.database ?? ""} />
          </Field>
          <Field title="Username">
            <Input onChange={(event) => onUpdate({ username: event.target.value })} value={form.username ?? ""} />
          </Field>
          <Field title="Credential Ref">
            <Input onChange={(event) => onUpdate({ credentialRef: event.target.value })} value={form.credentialRef ?? ""} />
          </Field>
        </>
      )}
      <div className="flex items-center gap-1">
        <Button disabled={savePending} size="sm" type="submit">
          <Save size={13} />
          Save
        </Button>
        <Button disabled={!selectedConnectionId || testPending} onClick={onTest} size="sm" type="button" variant="outline">
          <CheckCircle2 size={13} />
          {testPending ? "Connecting" : "Connect"}
        </Button>
        <IconButton disabled={!selectedConnectionId} label="Delete database connection" onClick={onDelete}>
          <Trash2 size={13} />
        </IconButton>
      </div>
      {error ? (
        <ErrorState className="min-h-[48px]">
          <DatabaseErrorDetails error={error} />
        </ErrorState>
      ) : null}
      {result && (
        <div className="flex items-center gap-2 text-[12px] text-[var(--u-color-text-muted)]">
          {result.ok ? <CheckCircle2 size={13} /> : <XCircle size={13} />}
          <span className="min-w-0 flex-1 truncate">{String(result.message)}</span>
          <StatusBadge tone={result.ok ? "success" : "warning"}>{result.ok ? "connected" : "failed"}</StatusBadge>
        </div>
      )}
    </form>
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
