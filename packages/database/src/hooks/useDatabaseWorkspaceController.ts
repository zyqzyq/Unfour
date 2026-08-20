import { type Dispatch, type FormEvent, type SetStateAction, useEffect } from "react";
import type { QueryClient } from "@tanstack/react-query";
import type {
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseSchema,
  DatabaseTable,
  DatabaseTestResult,
} from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { emptyDatabaseConnectionForm } from "../model/database-credentials";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { useDatabaseQueryWorkspaceActions } from "./useDatabaseQueryWorkspaceActions";
import { useDatabaseSchemaTreeActions } from "./useDatabaseSchemaTreeActions";
import { useDatabaseSqlRunner } from "./useDatabaseSqlRunner";
import { useDatabaseTableBrowse } from "./useDatabaseTableBrowse";
import { useQueryHistory } from "./useQueryHistory";
import { useSavedSql } from "./useSavedSql";
import type {
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  DatabaseQueryWorkspaceTab,
  DatabaseTableWorkspaceTab,
  SqlHistoryEntry,
} from "../model/types";

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
  const { loadCatalogNames, loadCatalogSchema, loadConnectionRoot } = useDatabaseSchemaTreeActions({
    catalogNamesByConn,
    queryClient,
    setCatalogNamesByConn,
    setTreeErrors,
    setTreeLoadingKeys,
    setTreeSchemaCache,
    treeLoadingKeys,
    treeSchemaCache,
    workspaceId,
  });

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
    setForm(emptyDatabaseConnectionForm(workspaceId));
    // Clear a previously failed save so its error doesn't leak into the new window.
    saveMutation.reset();
  }

  // Drop leftover connection id / credentialRef when the active workspace changes.
  useEffect(() => {
     
    setPassword("");
    setForm(emptyDatabaseConnectionForm(workspaceId));
    setEditorOpen(false);
    setTestResult(null);
    saveMutation.reset();
  }, [workspaceId]);

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

  const {
    applyPendingTableChanges,
    applyTableFilter,
    applyTableSort,
    browseMutation,
    browseTablePage,
    browsingRef,
    cancelledBrowseRequestsRef,
    previewSelectedTable,
    rowMutation,
  } = useDatabaseTableBrowse({
    activeTableTab,
    databaseTabs,
    selectedConnectionId,
    selectedTable,
    setConnectionState,
    setSelectedTable,
    selectConnection,
    t,
    workspaceId,
  });

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

  const { clearSql, runSql, selectQueryResult, sqlRunning, stopQuery } = useDatabaseSqlRunner({
    activeQueryTab,
    browseMutation,
    browsingRef,
    cancelledBrowseRequestsRef,
    databaseTabs,
    recordFailedHistory,
    recordSuccessfulHistory,
    setConnectionState,
    t,
    workspaceId,
  });

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

  function refreshActiveSchema() {
    const connectionId = activeTableTab?.connectionId ?? activeQueryTab?.connectionId ?? selectedConnectionId;
    const connection = connections.find((item) => item.id === connectionId);
    if (connection) {
      refreshConnectionSchema(connection);
    }
  }

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
