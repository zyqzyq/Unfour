import { Plug, Save } from "lucide-react";
import { FormEvent, type ReactNode, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import {
  createCredential,
  deleteDatabaseConnection,
  getDatabaseSchema,
  listDatabaseCatalogs,
  mutateDatabaseRow,
  rotateCredential,
  saveDatabaseConnection,
  testDatabaseConnection,
  testDatabaseConnectionInput,
} from "@unfour/command-client";
import type {
  DatabaseCellValue,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseSchema,
  DatabaseTable,
  DatabaseTestResult,
  SavedSql,
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
  Input,
  Select,
  useI18n,
} from "@unfour/ui";
import { DatabaseSidebar } from "./components/DatabaseSidebar";
import { DatabaseErrorDetails } from "./components/DatabaseErrorDetails";
import { DatabaseTestResultDialog } from "./components/DatabaseTestResultDialog";
import { DatabaseModuleToolbar } from "./components/DatabaseModuleToolbar";
import { DatabaseStatusBar } from "./components/DatabaseStatusBar";
import { DatabaseConnectionErrorBanner } from "./components/DatabaseConnectionErrorBanner";
import { DatabaseWorkspace } from "./components/DatabaseWorkspace";
import { useDatabaseConnections } from "./hooks/useDatabaseConnections";
import { databaseTableTabId, useDatabaseTabs } from "./hooks/useDatabaseTabs";
import { useDatabaseCatalogs } from "./hooks/useDatabaseCatalogs";
import { useQueryHistory } from "./hooks/useQueryHistory";
import { useSavedSql } from "./hooks/useSavedSql";
import { useSchemaTree } from "./hooks/useSchemaTree";
import { useSqlExecution } from "./hooks/useSqlExecution";
import { useTableData } from "./hooks/useTableData";
import { useTableStructure } from "./hooks/useTableStructure";
import { buildDatabaseTree, databaseTableTreeId, type DatabaseTreeModel } from "./model/database-tree";
import { EMPTY_CONNECTION_STATES, useDatabaseConnectionStore } from "./model/database-connection-state";
import type {
  DatabaseQueryWorkspaceTab,
  DatabaseConnectionSessionState,
  DatabaseConnectionStatus,
  DatabaseTableWorkspaceTab,
  SqlHistoryEntry,
  TableEditing,
  TableQueryState,
} from "./model/types";
import { emptyTableQuery } from "./model/types";
import { describeDatabaseError, formatDatabaseError, isConfirmationRequired } from "./result-utils";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;
const MAX_HISTORY_ENTRIES = 25;

export function DatabasePage({
  onShellSidebarChange,
  onShellStatusBarChange,
  statusBarRightAccessory,
  workspaceName,
  workspaceId,
}: {
  onShellSidebarChange?: (sidebar: ReactNode | null) => void;
  onShellStatusBarChange?: (statusBar: ReactNode | null) => void;
  statusBarRightAccessory?: ReactNode;
  workspaceName?: string;
  workspaceId: string;
}) {
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
  const filterDebounceRef = useRef<number | null>(null);
  // Set when the user stops a running query so a late-arriving backend result
  // (the statement keeps running server-side until it finishes or times out) is
  // ignored instead of replacing the cancelled state.
  const cancelledRef = useRef(false);
  const executingRef = useRef<{ connectionId: string | null; sql: string; tabId: string } | null>(null);
  const browsingRef = useRef<{ connectionId: string; table: DatabaseTable; tabId: string } | null>(null);
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
  const savedSqlByConnection = useMemo(() => {
    const map: Record<string, SavedSql[]> = {};
    for (const item of savedSqlQuery.saved) {
      if (!item.connectionId) {
        continue;
      }
      (map[item.connectionId] ??= []).push(item);
    }
    return map;
  }, [savedSqlQuery.saved]);
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

  const executeMutation = useSqlExecution({
    connectionId: activeQueryTab?.connectionId ?? null,
    onConfirmationRequired: (required) => {
      const tabId = executingRef.current?.tabId ?? activeQueryTab?.id;
      if (tabId) {
        databaseTabs.updateQueryTab(tabId, { pendingConfirmation: required });
      }
    },
    onError: (error) => {
      const execution = executingRef.current;
      if (execution) {
        databaseTabs.updateQueryTab(execution.tabId, {
          error,
          loading: false,
          resultTab: "results",
        });
      }
      if (isConfirmationRequired(error)) {
        return;
      }

      recordFailedHistory(error, execution);
      const description = describeDatabaseError(error);
      if (execution?.connectionId && ["connection", "network", "permission"].includes(description.category)) {
        setConnectionState(execution.connectionId, {
          message: description.message,
          status: "failed",
        });
      }
    },
    onExecuteStart: () => {
      cancelledRef.current = false;
      const execution = executingRef.current;
      if (execution) {
        databaseTabs.updateQueryTab(execution.tabId, {
          error: null,
          loading: true,
          resultTab: "results",
        });
      }
    },
    onSuccess: (result) => {
      if (cancelledRef.current) {
        return;
      }
      const execution = executingRef.current;
      if (execution) {
        databaseTabs.updateQueryTab(execution.tabId, {
          error: null,
          loading: false,
          pendingConfirmation: false,
          result,
          resultTab: "results",
        });
      }
      if (execution?.connectionId) {
        setConnectionState(execution.connectionId, {
          message: t("database.query.completed", {
            durationMs: result.durationMs,
          }),
          status: "connected",
        });
      }
      recordSuccessfulHistory(result, execution);
    },
    workspaceId,
  });

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

  const rowMutation = useMutation({
    mutationFn: mutateDatabaseRow,
    onSuccess: () => {
      // Re-read the current page so the grid reflects the committed change.
      refreshTablePage();
    },
    onError: (error) => {
      if (activeTableTab) {
        databaseTabs.updateTableTab(activeTableTab.id, { error });
      }
    },
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
    executeMutation.reset();
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

  function mutateRow(
    operation: "insert" | "update" | "delete",
    values: DatabaseCellValue[],
    primaryKey: DatabaseCellValue[],
  ) {
    if (!activeTableTab) {
      return;
    }
    databaseTabs.updateTableTab(activeTableTab.id, { error: null });
    rowMutation.mutate({
      workspaceId,
      connectionId: activeTableTab.connectionId,
      catalog: activeTableTab.table.catalog,
      schema: activeTableTab.table.schema,
      tableName: activeTableTab.table.name,
      operation,
      values,
      primaryKey,
    });
  }

  function runSql(overrideSql?: string) {
    executeMutation.reset();
    browseMutation.reset();

    if (!activeQueryTab) {
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

    // Run the highlighted statement when the editor reports a non-empty
    // selection; otherwise fall back to the full editor contents.
    const effectiveSql = overrideSql && overrideSql.trim() ? overrideSql : activeQueryTab.sql;
    if (!effectiveSql.trim()) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: {
          code: "VALIDATION_ERROR",
          message: t("database.errors.sqlEmpty"),
        },
        resultTab: "results",
      });
      return;
    }

    executingRef.current = {
      connectionId: activeQueryTab.connectionId,
      sql: effectiveSql,
      tabId: activeQueryTab.id,
    };
    databaseTabs.updateQueryTab(activeQueryTab.id, { error: null, resultTab: "results" });
    executeMutation.mutate({
      confirmMutation: activeQueryTab.pendingConfirmation,
      sql: effectiveSql,
      catalog: activeQueryTab.catalog,
      schema: activeQueryTab.schema,
    });
  }

  function clearSql() {
    if (!activeQueryTab) {
      return;
    }
    databaseTabs.updateQueryTab(activeQueryTab.id, {
      error: null,
      pendingConfirmation: false,
      sql: "",
    });
    executeMutation.reset();
  }

  // Stop a running query/preview. The mutation is abandoned so the UI is
  // responsive immediately; the statement keeps running server-side until it
  // finishes or hits its timeout, but its late result is ignored.
  function stopQuery() {
    const wasRunning = executeMutation.isPending || browseMutation.isPending;
    if (!wasRunning) {
      return;
    }
    cancelledRef.current = true;
    executeMutation.reset();
    browseMutation.reset();
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

  function startNewQuery(
    connectionId = selectedConnectionId ?? activeQueryTab?.connectionId ?? activeTableTab?.connectionId ?? null,
  ) {
    const tabId = databaseTabs.openQueryTab({ connectionId });
    if (connectionId) {
      setSelectedDatabaseConnection(connectionId);
    }
    setSelectedTable(null);
    return tabId;
  }

  function showQueryHistory() {
    const tabId = activeQueryTab?.id ?? databaseTabs.openQueryTab({ connectionId: selectedConnectionId });
    databaseTabs.updateQueryTab(tabId, { resultTab: "history" });
  }

  function recordSuccessfulHistory(
    result: DatabaseQueryResult,
    execution: { connectionId: string | null; sql: string } | null,
  ) {
    appendHistory({
      affectedRows: result.affectedRows,
      classification: result.safety.classification,
      connectionId: execution?.connectionId ?? null,
      connectionName: connectionNameForHistory(execution?.connectionId),
      durationMs: result.durationMs,
      rowCount: result.rows.length,
      sql: execution?.sql ?? "",
      status: "success",
    });
  }

  function recordFailedHistory(error: unknown, execution: { connectionId: string | null; sql: string } | null) {
    appendHistory({
      connectionId: execution?.connectionId ?? null,
      connectionName: connectionNameForHistory(execution?.connectionId),
      error: formatDatabaseError(error),
      sql: execution?.sql ?? "",
      status: "failed",
    });
  }

  function connectionNameForHistory(connectionId: string | null | undefined) {
    return connections.find((connection) => connection.id === connectionId)?.name ?? t("database.query.unknownConnection");
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
    const connectionId = connections.some((connection) => connection.id === entry.connectionId)
      ? entry.connectionId
      : null;
    databaseTabs.openQueryTab({
      connectionId,
      sql: entry.sql,
    });
    if (connectionId) {
      setSelectedDatabaseConnection(connectionId);
    }
    setSelectedTable(null);
  }

  // Open a saved SQL snippet from the sidebar tree into a fresh query tab.
  // Mirrors loadHistoryEntry: the connection id is honored only when the
  // owning connection still exists, otherwise the snippet opens without one.
  function openSavedSql(item: SavedSql) {
    const connectionId =
      item.connectionId && connections.some((connection) => connection.id === item.connectionId)
        ? item.connectionId
        : null;
    databaseTabs.openQueryTab({
      connectionId,
      sql: item.sql,
    });
    if (connectionId) {
      setSelectedDatabaseConnection(connectionId);
    }
    setSelectedTable(null);
  }

  function deleteSavedSql(item: SavedSql) {
    void savedSqlQuery.remove(item.id);
  }

  // Load generated SQL (e.g. from a table context-menu action) into a fresh editor tab.
  function loadSqlIntoEditor(connectionId: string, generatedSql: string, table?: DatabaseTable) {
    databaseTabs.openQueryTab({
      catalog: table?.catalog ?? null,
      connectionId,
      schema: table?.schema ?? null,
      sql: generatedSql,
    });
    setSelectedDatabaseConnection(connectionId);
    setSelectedTable(null);
  }

  function setActiveTabError(error: unknown) {
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, { error, resultTab: "results" });
      return;
    }
    if (activeTableTab) {
      databaseTabs.updateTableTab(activeTableTab.id, { error });
    }
  }

  function selectQueryConnection(connectionId: string | null) {
    setSelectedDatabaseConnection(connectionId);
    setSelectedTable(null);
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        catalog: null,
        connectionId,
        error: null,
        pendingConfirmation: false,
        schema: null,
      });
    }
  }

  function selectDatabaseTab(tabId: string) {
    const tab = databaseTabs.tabs.find((item) => item.id === tabId);
    databaseTabs.setActiveTabId(tabId);
    if (!tab) {
      return;
    }
    setSelectedDatabaseConnection(tab.connectionId);
    if (tab.kind === "table") {
      setSelectedTable(tab.table);
    } else {
      setSelectedTable(null);
    }
  }

  function designTable(connectionId: string, table: DatabaseTable) {
    const tabId = databaseTabs.openTableTab(connectionId, table, "structure");
    databaseTabs.updateTableTab(tabId, { segment: "structure" });
    setSelectedDatabaseConnection(connectionId);
    setSelectedTable(table);
  }

  function handleTablePageChange(pageIndex: number, pageSize: number) {
    if (!activeTableTab) {
      return;
    }
    browseTablePage(activeTableTab.connectionId, activeTableTab.table, pageIndex, pageSize);
  }

  function handleSelectResultTab(tab: DatabaseQueryWorkspaceTab["resultTab"]) {
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, { resultTab: tab });
    }
  }

  function handleSelectStructureTab(tab: DatabaseTableWorkspaceTab["structureTab"]) {
    if (activeTableTab) {
      databaseTabs.updateTableTab(activeTableTab.id, { structureTab: tab });
    }
  }

  function handleSelectTableSegment(segment: DatabaseTableWorkspaceTab["segment"]) {
    if (activeTableTab) {
      databaseTabs.updateTableTab(activeTableTab.id, { segment });
    }
  }

  function updateActiveSql(sql: string) {
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: null,
        pendingConfirmation: false,
        sql,
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
  const executePending = executeMutation.isPending || browseMutation.isPending || rowMutation.isPending;
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
  const primaryKeyColumns = (activeTableTab?.table.columns ?? [])
    .filter((column) => column.primaryKey)
    .map((column) => column.name);
  const tableEditing: TableEditing | null =
    activeTableTab &&
    activeTableTab.tableView &&
    activeTableStatus === "connected" &&
    primaryKeyColumns.length > 0 &&
    !activeTableConnection?.readOnly
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

function DatabaseConnectionDialog({
  canTest,
  error,
  form,
  onOpenChange,
  onPasswordChange,
  onSubmit,
  onTest,
  onUpdate,
  open,
  password,
  savePending,
  testPending,
  children,
}: {
  canTest: boolean;
  error: unknown;
  form: DatabaseConnectionInput;
  onOpenChange: (open: boolean) => void;
  onPasswordChange: (value: string) => void;
  onSubmit: (event: FormEvent) => void;
  onTest: () => void;
  onUpdate: (patch: Partial<DatabaseConnectionInput>) => void;
  open: boolean;
  password: string;
  savePending: boolean;
  testPending: boolean;
  children?: ReactNode;
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
                    sslMode: event.target.value === "sqlite" ? null : form.sslMode,
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
            <label className="flex items-start gap-2 pt-1">
              <input
                checked={Boolean(form.readOnly)}
                className="mt-0.5"
                onChange={(event) => onUpdate({ readOnly: event.target.checked })}
                type="checkbox"
              />
              <span className="min-w-0">
                <span className="block text-[12px] font-medium text-[var(--u-color-text)]">
                  {t("database.fields.readOnly")}
                </span>
                <span className="block text-[11px] text-[var(--u-color-text-soft)]">
                  {t("database.fields.readOnlyHint")}
                </span>
              </span>
            </label>
            {error ? (
              <ErrorState className="min-h-[48px]">
                <DatabaseErrorDetails error={error} />
              </ErrorState>
            ) : null}
          </DialogBody>
          <DialogFooter>
            <Button className="mr-auto" disabled={!canTest || testPending} onClick={onTest} size="sm" type="button" variant="outline">
              <Plug size={13} />
              {testPending ? t("database.connection.testing") : t("database.connection.test")}
            </Button>
            <Button onClick={() => onOpenChange(false)} size="sm" type="button" variant="ghost">
              {t("common.confirm.cancel")}
            </Button>
            <Button disabled={savePending} size="sm" type="submit">
              <Save size={13} />
              {t("common.actions.save")}
            </Button>
          </DialogFooter>
        </form>
        {children}
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

function normalizeQueryContext(
  current: Pick<DatabaseQueryWorkspaceTab, "catalog" | "schema">,
  treeModel: DatabaseTreeModel,
) {
  const currentCatalog = treeModel.catalogs.find((catalog) => catalog.key === (current.catalog ?? ""));
  const fallbackCatalog = currentCatalog ?? treeModel.catalogs[0];
  if (!fallbackCatalog) {
    return { catalog: null, schema: null };
  }

  const catalog = fallbackCatalog.key || null;
  if (!fallbackCatalog.hasSchemaLevel) {
    return { catalog, schema: null };
  }

  const currentSchema = fallbackCatalog.schemas.find((schema) => schema.key === (current.schema ?? ""));
  const fallbackSchema = currentSchema ?? fallbackCatalog.schemas[0];
  return {
    catalog,
    schema: fallbackSchema?.key || null,
  };
}
