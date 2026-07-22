import { type Dispatch, type FormEvent, type SetStateAction, useEffect, useRef, useState } from "react";
import type { QueryClient } from "@tanstack/react-query";
import {
  getDatabaseSchema,
  listDatabaseCatalogs,
} from "@unfour/command-client";
import type {
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTable,
  DatabaseTestResult,
} from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { databaseTableTabId, useDatabaseTabs } from "./useDatabaseTabs";
import { useDatabaseQueryWorkspaceActions } from "./useDatabaseQueryWorkspaceActions";
import { useQueryHistory } from "./useQueryHistory";
import { useSavedSql } from "./useSavedSql";
import { useTableData } from "./useTableData";
import { useTableRowMutations } from "./useTableRowMutations";
import { resolveExecutableStatements } from "../model/sql-statements";
import { executeSqlBatch, type SqlBatchState } from "../model/run-sql-batch";
import type {
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  DatabaseQueryWorkspaceTab,
  DatabaseTableWorkspaceTab,
  RunSqlOptions,
  SqlHistoryEntry,
  TableQueryState,
} from "../model/types";
import { emptyTableQuery } from "../model/types";
import { describeDatabaseError, formatDatabaseError } from "../result-utils";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;

type SaveMutation = {
  mutate: (variables: { input: DatabaseConnectionInput; secret: string }) => void;
  reset: () => void;
};

type TestMutation = {
  mutate: (connectionId: string) => void;
};

type TestInputMutation = {
  mutate: (variables: { input: DatabaseConnectionInput; secret: string | null }) => void;
};

type DatabaseWorkspaceControllerOptions = {
  activeQueryTab: DatabaseQueryWorkspaceTab | null;
  activeTableTab: DatabaseTableWorkspaceTab | null;
  catalogNamesByConn: Record<string, string[]>;
  connectionStates: Record<string, DatabaseConnectionSessionState>;
  connections: DatabaseConnection[];
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  form: DatabaseConnectionInput;
  maxHistoryEntries: number;
  password: string;
  queryClient: QueryClient;
  queryHistoryQuery: ReturnType<typeof useQueryHistory>;
  saveMutation: SaveMutation;
  savedSqlQuery: ReturnType<typeof useSavedSql>;
  selectedConnection: DatabaseConnection | null;
  selectedConnectionId: string | null;
  selectedConnectionStatus: DatabaseConnectionStatus;
  selectedTable: DatabaseTable | null;
  setCatalogNamesByConn: Dispatch<SetStateAction<Record<string, string[]>>>;
  setConnectionState: (
    connectionId: string,
    patch: Partial<DatabaseConnectionSessionState>,
  ) => void;
  setEditorOpen: Dispatch<SetStateAction<boolean>>;
  setForm: Dispatch<SetStateAction<DatabaseConnectionInput>>;
  setPassword: Dispatch<SetStateAction<string>>;
  setQueryHistory: Dispatch<SetStateAction<SqlHistoryEntry[]>>;
  setSelectedDatabaseConnection: (connectionId: string | null) => void;
  setSelectedTable: Dispatch<SetStateAction<DatabaseTable | null>>;
  setTestResult: Dispatch<SetStateAction<DatabaseTestResult | null>>;
  setTreeErrors: Dispatch<SetStateAction<Record<string, string>>>;
  setTreeLoadingKeys: Dispatch<SetStateAction<string[]>>;
  setTreeSchemaCache: Dispatch<SetStateAction<Record<string, DatabaseSchema>>>;
  t: ReturnType<typeof useI18n>["t"];
  testInputMutation: TestInputMutation;
  testMutation: TestMutation;
  treeLoadingKeys: string[];
  treeSchemaCache: Record<string, DatabaseSchema>;
  workspaceId: string;
};

export function useDatabaseWorkspaceController({
  activeQueryTab,
  activeTableTab,
  catalogNamesByConn,
  connectionStates,
  connections,
  databaseTabs,
  form,
  maxHistoryEntries,
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
}: DatabaseWorkspaceControllerOptions) {
  const filterDebounceRef = useRef<number | null>(null);
  const cancelledRef = useRef(false);
  const executingRef = useRef<{ connectionId: string | null; sql: string; tabId: string } | null>(null);
  const browsingRef = useRef<{ connectionId: string; table: DatabaseTable; tabId: string } | null>(null);
  const batchRef = useRef<SqlBatchState | null>(null);
  const [sqlRunning, setSqlRunning] = useState(false);

  const browseMutation = useTableData({
    onBrowseStart: () => {
      cancelledRef.current = false;
      const browse = browsingRef.current;
      if (browse) {
        databaseTabs.updateTableTab(browse.tabId, {
          error: null,
          loading: true,
          segment: "data",
        });
      }
    },
    onSuccess: (browse) => {
      if (cancelledRef.current) {
        return;
      }
      const target = browsingRef.current;
      if (target) {
        databaseTabs.updateTableTab(target.tabId, {
          error: null,
          loading: false,
          queryResult: browse.result,
          segment: "data",
          tableView: {
            pageIndex: Math.floor(browse.offset / Math.max(1, browse.limit)),
            pageSize: browse.limit,
            readOnly: browse.readOnly,
            tableName: browse.tableName,
            totalRows: browse.totalRows,
          },
        });
        setConnectionState(target.connectionId, {
          message: t("database.query.previewLoaded", {
            count: browse.result.rows.length,
          }),
          status: "connected",
        });
      }
    },
    workspaceId,
  });

  const { applyPendingTableChanges, rowMutation } = useTableRowMutations({
    activeTableTab,
    databaseTabs,
    refreshTablePage,
    workspaceId,
  });

  useEffect(() => {
    const browse = browsingRef.current;
    if (!browse || !browseMutation.error) {
      return;
    }

    const description = describeDatabaseError(browseMutation.error);
    databaseTabs.updateTableTab(browse.tabId, { error: browseMutation.error, loading: false });
    if (["connection", "network", "permission"].includes(description.category)) {
      setConnectionState(browse.connectionId, {
        message: description.message,
        status: "failed",
      });
    }
  }, [browseMutation.error]);

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
        sslMode: form.sslMode ?? null,
      },
      secret: password,
    });
  }

  function selectConnection(connectionId: string | null) {
    setSelectedDatabaseConnection(connectionId);
    setTestResult(null);
    setSelectedTable(null);
  }

  // Load a connection's databases when its tree node is expanded: SQLite loads
  // its single file schema directly; PostgreSQL/MySQL load the database list.
  function loadConnectionRoot(connection: DatabaseConnection) {
    if (connection.driver === "sqlite") {
      loadCatalogSchema(connection.id, "");
      return;
    }
    loadCatalogNames(connection.id);
  }

  function loadCatalogNames(connectionId: string, options: { force?: boolean } = {}) {
    const key = `names::${connectionId}`;
    if ((!options.force && catalogNamesByConn[connectionId]) || treeLoadingKeys.includes(key)) {
      return;
    }
    setTreeLoadingKeys((current) => [...current, key]);
    queryClient
      .fetchQuery({
        queryKey: ["database-catalogs", workspaceId, connectionId],
        queryFn: () => listDatabaseCatalogs(workspaceId, connectionId),
      })
      .then((names) => {
        setCatalogNamesByConn((prev) => ({ ...prev, [connectionId]: names }));
        clearTreeError(key);
      })
      .catch((error) => setTreeError(key, error))
      .finally(() => setTreeLoadingKeys((current) => current.filter((item) => item !== key)));
  }

  // Lazily fetch a database (catalog) schema when its tree node is expanded.
  function loadCatalogSchema(connectionId: string, catalog: string, options: { force?: boolean } = {}) {
    const key = `${connectionId}::${catalog}`;
    if ((!options.force && treeSchemaCache[key]) || treeLoadingKeys.includes(key)) {
      return;
    }
    setTreeLoadingKeys((current) => [...current, key]);
    queryClient
      .fetchQuery({
        queryKey: ["database-schema", workspaceId, connectionId, catalog || null],
        queryFn: () => getDatabaseSchema(workspaceId, connectionId, catalog || null),
      })
      .then((data) => {
        setTreeSchemaCache((prev) => ({ ...prev, [key]: data }));
        clearTreeError(key);
      })
      .catch((error) => setTreeError(key, error))
      .finally(() => setTreeLoadingKeys((current) => current.filter((item) => item !== key)));
  }

  function setTreeError(key: string, error: unknown) {
    setTreeErrors((prev) => ({ ...prev, [key]: formatDatabaseError(error) }));
  }

  function clearTreeError(key: string) {
    setTreeErrors((prev) => {
      if (!(key in prev)) {
        return prev;
      }
      const next = { ...prev };
      delete next[key];
      return next;
    });
  }

  function connectConnection(connection: DatabaseConnection) {
    selectConnection(connection.id);
    setSelectedTable(null);
    testMutation.mutate(connection.id);
  }

  // Validate the dialog form against the backend without saving it. Mirrors the
  // SSH dialog's `canTest` gate: enough fields to attempt a connection, plus a
  // credential (typed password for a new connection, or the stored reference
  // when editing an existing one).
  const canTest = Boolean(form.name?.trim()) && (
    form.driver === "sqlite"
      ? Boolean(form.sqlitePath?.trim())
      : Boolean(form.host?.trim()) &&
        Boolean(form.port) &&
        (Boolean(form.credentialRef) || Boolean(password.trim()))
  );

  function testConnectionInput() {
    testInputMutation.mutate({ input: form, secret: password || null });
  }

  function disconnectConnection(connection: DatabaseConnection) {
    setConnectionState(connection.id, {
      message: t("database.connection.disconnected"),
      status: "disconnected",
    });
    if (connection.id === selectedConnectionId) {
      setTestResult(null);
    }
  }

  function newConnection() {
    selectConnection(null);
    setPassword("");
    setForm({ workspaceId, name: "", driver: "sqlite", sqlitePath: "" });
    // Clear a previously failed save so its error doesn't leak into the new window.
    saveMutation.reset();
  }

  function refreshConnectionsAndSchema() {
    queryClient.invalidateQueries({ queryKey: ["database-connections", workspaceId] });
    if (selectedConnection && selectedConnectionStatus !== "disconnected") {
      refreshConnectionSchema(selectedConnection);
    }
  }

  function refreshConnectionSchema(connection: DatabaseConnection) {
    const status = connectionStates[connection.id]?.status ?? "disconnected";
    if (connection.id !== selectedConnectionId) {
      selectConnection(connection.id);
    }

    if (status === "disconnected") {
      setActiveTabError({
        code: "VALIDATION_ERROR",
        message: t("database.connection.connectBeforeRefresh"),
      });
      return;
    }

    queryClient.invalidateQueries({ queryKey: ["database-schema", workspaceId, connection.id] });
    queryClient.invalidateQueries({ queryKey: ["database-catalogs", workspaceId, connection.id] });

    const prefix = `${connection.id}::`;
    const loadedCatalogs = Object.keys(treeSchemaCache)
      .filter((key) => key.startsWith(prefix))
      .map((key) => key.slice(prefix.length));
    if (connection.driver === "sqlite") {
      loadCatalogSchema(connection.id, "", { force: true });
    } else {
      loadCatalogNames(connection.id, { force: true });
      for (const catalog of loadedCatalogs) {
        loadCatalogSchema(connection.id, catalog, { force: true });
      }
    }
  }

  function selectTable(connectionId: string, table: DatabaseTable) {
    // Single click: lightweight selection only (Navicat convention).
    // Does NOT switch Tab or load data -- that requires a double-click.
    if (connectionId !== selectedConnectionId) {
      selectConnection(connectionId);
    }
    setSelectedTable(table);
  }

  function changeQueryContext(patch: { catalog?: string | null; schema?: string | null }) {
    if (!activeQueryTab) {
      return;
    }
    databaseTabs.updateQueryTab(activeQueryTab.id, (tab) => {
      const next = { catalog: tab.catalog, schema: tab.schema, ...patch };
      // Switching catalog invalidates a schema from the previous catalog.
      if (patch.catalog !== undefined && patch.catalog !== tab.catalog) {
        next.schema = null;
      }
      return { catalog: next.catalog, schema: next.schema };
    });
  }

  function browseTablePage(
    connectionId: string,
    table: DatabaseTable,
    pageIndex: number,
    pageSize: number,
    query?: TableQueryState,
  ) {
    const existingTab = databaseTabs.tabs.find((tab) => tab.id === databaseTableTabId(connectionId, table));
    const effectiveQuery =
      query ?? (existingTab?.kind === "table" ? existingTab.tableQuery : { ...emptyTableQuery });
    const tabId = databaseTabs.openTableTab(connectionId, table, "data");
    if (connectionId !== selectedConnectionId) {
      selectConnection(connectionId);
    }
    setSelectedTable(table);
    databaseTabs.updateTableTab(tabId, {
      error: null,
      segment: "data",
      tableQuery: effectiveQuery,
    });
    browsingRef.current = { connectionId, table, tabId };
    browseMutation.reset();
    browseMutation.mutate({
      connectionId,
      catalog: table.catalog,
      pageIndex: Math.max(0, pageIndex),
      pageSize,
      schema: table.schema,
      tableName: table.name,
      orderBy: effectiveQuery.orderBy,
      orderDescending: effectiveQuery.orderDescending,
      filter: effectiveQuery.filter || null,
    });
  }

  // Cycle a column through ascending -> descending -> unsorted, re-querying the
  // first page server-side each time.
  function applyTableSort(column: string) {
    if (!activeTableTab) {
      return;
    }
    const current = activeTableTab.tableQuery;
    let next: { orderBy: string | null; orderDescending: boolean; filter: string };
    if (current.orderBy !== column) {
      next = { ...current, orderBy: column, orderDescending: false };
    } else if (!current.orderDescending) {
      next = { ...current, orderDescending: true };
    } else {
      next = { ...current, orderBy: null, orderDescending: false };
    }
    browseTablePage(
      activeTableTab.connectionId,
      activeTableTab.table,
      0,
      activeTableTab.tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE,
      next,
    );
  }

  // Debounce the cross-column filter so typing does not fire a query per key.
  function applyTableFilter(text: string) {
    if (!activeTableTab) {
      return;
    }
    const next = { ...activeTableTab.tableQuery, filter: text };
    databaseTabs.updateTableTab(activeTableTab.id, { tableQuery: next });
    if (filterDebounceRef.current) {
      window.clearTimeout(filterDebounceRef.current);
    }
    const connectionId = activeTableTab.connectionId;
    const table = activeTableTab.table;
    const pageSize = activeTableTab.tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE;
    filterDebounceRef.current = window.setTimeout(() => {
      browseTablePage(connectionId, table, 0, pageSize, next);
    }, 350);
  }

  function previewSelectedTable() {
    if (!selectedConnectionId || !selectedTable) {
      return;
    }
    browseTablePage(selectedConnectionId, selectedTable, 0, activeTableTab?.tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE);
  }

  function refreshTablePage() {
    if (activeTableTab?.tableView) {
      browseTablePage(
        activeTableTab.connectionId,
        activeTableTab.table,
        activeTableTab.tableView.pageIndex,
        activeTableTab.tableView.pageSize,
      );
    }
  }

  const {
    clearQueryHistory,
    deleteSavedSql,
    designTable,
    handleSelectResultTab,
    handleSelectStructureTab,
    handleSelectTableSegment,
    handleTablePageChange,
    loadHistoryEntry,
    loadSqlIntoEditor,
    openSavedSql,
    recordFailedHistory,
    recordSuccessfulHistory,
    selectDatabaseTab,
    selectQueryConnection,
    setActiveTabError,
    showQueryHistory,
    startNewQuery,
    updateActiveSql,
  } = useDatabaseQueryWorkspaceActions({
    activeQueryTab,
    activeTableTab,
    browseTablePage,
    connections,
    databaseTabs,
    maxHistoryEntries,
    queryHistoryQuery,
    savedSqlQuery,
    selectedConnectionId,
    setQueryHistory,
    setSelectedDatabaseConnection,
    setSelectedTable,
    t,
  });

  function normalizeRunOptions(options?: string | RunSqlOptions): RunSqlOptions {
    if (typeof options === "string") {
      return { mode: "current", sql: options };
    }
    return options ?? { mode: "current" };
  }

  function applyQueryResults(tabId: string, collected: DatabaseQueryResult[], error: unknown = null) {
    const activeResultIndex = collected.length > 0 ? collected.length - 1 : 0;
    databaseTabs.updateQueryTab(tabId, {
      activeResultIndex,
      error,
      loading: false,
      result: collected[activeResultIndex] ?? null,
      results: collected,
      resultTab: "results",
    });
  }

  async function runSqlBatch(batch: SqlBatchState, confirmMutation: boolean) {
    cancelledRef.current = false;
    setSqlRunning(true);
    batchRef.current = batch;
    databaseTabs.updateQueryTab(batch.tabId, {
      error: null,
      loading: true,
      pendingConfirmation: false,
      resultTab: "results",
    });

    try {
      const outcome = await executeSqlBatch(batch, confirmMutation, {
        cancelled: () => cancelledRef.current,
        onConfirmationRequired: (paused, collected, error) => {
          batchRef.current = paused;
          executingRef.current = {
            connectionId: paused.connectionId,
            sql: paused.statements[paused.nextIndex] ?? "",
            tabId: paused.tabId,
          };
          databaseTabs.updateQueryTab(paused.tabId, {
            activeResultIndex: collected.length > 0 ? collected.length - 1 : 0,
            error,
            loading: false,
            pendingConfirmation: true,
            result: collected[collected.length - 1] ?? null,
            results: collected,
            resultTab: "results",
          });
        },
        onError: (current, collected, sql, error) => {
          executingRef.current = {
            connectionId: current.connectionId,
            sql,
            tabId: current.tabId,
          };
          applyQueryResults(current.tabId, collected, error);
          databaseTabs.updateQueryTab(current.tabId, { pendingConfirmation: false });
          recordFailedHistory(error, {
            connectionId: current.connectionId,
            sql,
          });
          const description = describeDatabaseError(error);
          if (["connection", "network", "permission"].includes(description.category)) {
            setConnectionState(current.connectionId, {
              message: description.message,
              status: "failed",
            });
          }
          batchRef.current = null;
        },
        onStatementSuccess: (current, collected, sql, result) => {
          executingRef.current = {
            connectionId: current.connectionId,
            sql,
            tabId: current.tabId,
          };
          batchRef.current = { ...current, collected, nextIndex: current.nextIndex + 1 };
          applyQueryResults(current.tabId, collected);
          recordSuccessfulHistory(result, {
            connectionId: current.connectionId,
            sql,
          });
          setConnectionState(current.connectionId, {
            message: t("database.query.completed", { durationMs: result.durationMs }),
            status: "connected",
          });
        },
        onSuccess: (current, collected) => {
          applyQueryResults(current.tabId, collected);
          databaseTabs.updateQueryTab(current.tabId, { pendingConfirmation: false });
          batchRef.current = null;
        },
        workspaceId,
      });

      if (outcome === "cancelled") {
        // stopQuery already wrote the cancelled error onto the tab.
        return;
      }
    } finally {
      setSqlRunning(false);
    }
  }

  function runSql(options?: string | RunSqlOptions) {
    browseMutation.reset();

    if (!activeQueryTab) {
      return;
    }

    const request = normalizeRunOptions(options);
    const pendingBatch =
      activeQueryTab.pendingConfirmation &&
      batchRef.current &&
      batchRef.current.tabId === activeQueryTab.id
        ? batchRef.current
        : null;

    if ((request.resume || activeQueryTab.pendingConfirmation) && pendingBatch) {
      void runSqlBatch(pendingBatch, true);
      return;
    }

    if (!activeQueryTab.connectionId) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: {
          code: "VALIDATION_ERROR",
          message: t("database.errors.selectBeforeRun"),
        },
        resultTab: "results",
      });
      return;
    }

    const statements = resolveExecutableStatements(activeQueryTab.sql, {
      mode: request.mode ?? "current",
      sql: request.sql,
      cursorOffset: request.cursorOffset,
    });

    if (!statements.length) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: {
          code: "VALIDATION_ERROR",
          message: t("database.errors.sqlEmpty"),
        },
        resultTab: "results",
      });
      return;
    }

    void runSqlBatch(
      {
        catalog: activeQueryTab.catalog,
        collected: [],
        connectionId: activeQueryTab.connectionId,
        nextIndex: 0,
        schema: activeQueryTab.schema,
        statements,
        tabId: activeQueryTab.id,
      },
      false,
    );
  }

  function clearSql() {
    if (!activeQueryTab) {
      return;
    }
    batchRef.current = null;
    databaseTabs.updateQueryTab(activeQueryTab.id, {
      activeResultIndex: 0,
      error: null,
      pendingConfirmation: false,
      result: null,
      results: [],
      sql: "",
    });
  }

  function selectQueryResult(index: number) {
    if (!activeQueryTab) {
      return;
    }
    const result = activeQueryTab.results[index];
    if (!result) {
      return;
    }
    databaseTabs.updateQueryTab(activeQueryTab.id, {
      activeResultIndex: index,
      result,
    });
  }

  // Stop a running query/preview. The in-flight statement keeps running
  // server-side until it finishes or hits its timeout, but late results are ignored.
  function stopQuery() {
    const wasRunning = sqlRunning || browseMutation.isPending;
    if (!wasRunning) {
      return;
    }
    cancelledRef.current = true;
    browseMutation.reset();
    setSqlRunning(false);
    const cancelledError = { code: "QUERY_CANCELLED", message: t("database.query.cancelled") };
    if (executingRef.current) {
      databaseTabs.updateQueryTab(executingRef.current.tabId, {
        error: cancelledError,
        loading: false,
        pendingConfirmation: false,
        resultTab: "results",
      });
    }
    if (browsingRef.current) {
      databaseTabs.updateTableTab(browsingRef.current.tabId, {
        error: cancelledError,
        loading: false,
      });
    }
    const connectionId = executingRef.current?.connectionId ?? browsingRef.current?.connectionId;
    if (connectionId) {
      setConnectionState(connectionId, {
        message: t("database.query.cancelled"),
        status: "connected",
      });
    }
  }

  function refreshActiveSchema() {
    const connectionId = activeTableTab?.connectionId ?? activeQueryTab?.connectionId ?? selectedConnectionId;
    const connection = connections.find((item) => item.id === connectionId);
    if (connection) {
      refreshConnectionSchema(connection);
    }
  }

  function handleNewConnection() {
    newConnection();
    setEditorOpen(true);
  }

  function handleEditConnection(connection: DatabaseConnection) {
    selectConnection(connection.id);
    // Clear a previously failed save so its error doesn't leak into this edit window.
    saveMutation.reset();
    setEditorOpen(true);
  }

  // Keep the latest handlers in a ref (render-time write, matching the existing
  // prevSelectedConnectionIdRef pattern) so the pushed shell sidebar can use

  return {
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
  };
}
