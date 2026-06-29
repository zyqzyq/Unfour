import { CheckCircle2, Save, Trash2, XCircle } from "lucide-react";
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
} from "@unfour/command-client";
import type {
  DatabaseCellValue,
  DatabaseConnection,
  DatabaseConnectionInput,
  DatabaseQueryResult,
  DatabaseSchema,
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
import { useDatabaseCatalogs } from "./hooks/useDatabaseCatalogs";
import { useQueryHistory } from "./hooks/useQueryHistory";
import { useSchemaTree } from "./hooks/useSchemaTree";
import { useSqlExecution } from "./hooks/useSqlExecution";
import { useTableData } from "./hooks/useTableData";
import { useTableStructure } from "./hooks/useTableStructure";
import { defaultSql } from "./model/database-state";
import { buildDatabaseTree, databaseTableTreeId } from "./model/database-tree";
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
  const [queryContext, setQueryContext] = useState<{ catalog: string | null; schema: string | null }>({
    catalog: null,
    schema: null,
  });
  // Server-side sort/filter for the table data view, pushed into browse_table so
  // it applies to the whole table rather than only the loaded page.
  const [tableQuery, setTableQuery] = useState<{
    orderBy: string | null;
    orderDescending: boolean;
    filter: string;
  }>({ orderBy: null, orderDescending: false, filter: "" });
  const filterDebounceRef = useRef<number | null>(null);
  const emptyTableQuery = { orderBy: null, orderDescending: false, filter: "" } as const;
  // Set when the user stops a running query so a late-arriving backend result
  // (the statement keeps running server-side until it finishes or times out) is
  // ignored instead of replacing the cancelled state.
  const cancelledRef = useRef(false);
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
      treeModel.catalogs.find((catalog) => catalog.key === queryContext.catalog) ??
      treeModel.catalogs[0];
    if (!activeCatalog?.hasSchemaLevel) {
      return [];
    }
    return activeCatalog.schemas.map((schema) => schema.key).filter((key) => key !== "");
  }, [treeModel, queryContext.catalog]);
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

  // Keep the query context pointed at a valid catalog/schema as the schema
  // loads or changes. Preserves a still-valid user selection; otherwise falls
  // back to the first catalog and (for PostgreSQL) its first schema.
  useEffect(() => {
    if (!treeModel) {
      return;
    }
    // eslint-disable-next-line react-hooks/set-state-in-effect -- re-deriving context defaults when the loaded schema changes
    setQueryContext((current) => {
      const activeCatalog =
        treeModel.catalogs.find((catalog) => catalog.key === current.catalog) ??
        treeModel.catalogs.find((catalog) => catalog.key !== "") ??
        treeModel.catalogs[0] ??
        null;
      const catalog = activeCatalog && activeCatalog.key !== "" ? activeCatalog.key : null;

      let schema: string | null = null;
      if (activeCatalog?.hasSchemaLevel) {
        const keep = activeCatalog.schemas.some((node) => node.key === current.schema);
        schema = keep
          ? current.schema
          : (activeCatalog.schemas.find((node) => node.key !== "")?.key ?? null);
      }

      if (catalog === current.catalog && schema === current.schema) {
        return current;
      }
      return { catalog, schema };
    });
  }, [treeModel]);

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
      cancelledRef.current = false;
      setClientError(null);
      setTableView(null);
      layout.setActiveTabId("query");
      layout.setResultTab("results");
    },
    onSuccess: (result) => {
      if (cancelledRef.current) {
        return;
      }
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
    onBrowseStart: () => {
      cancelledRef.current = false;
      setClientError(null);
      setPendingSqlConfirmation(false);
      layout.setActiveTabId("table");
      layout.setTableSegment("data");
      layout.setResultTab("results");
    },
    onSuccess: (browse) => {
      if (cancelledRef.current) {
        return;
      }
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
    setTableQuery(emptyTableQuery);
    // Drop the previous datasource's context; the schema-load effect repopulates
    // a valid default for the newly selected connection. The per-connection tree
    // caches are kept so other connections stay expanded.
    setQueryContext({ catalog: null, schema: null });
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

  function loadCatalogNames(connectionId: string) {
    const key = `names::${connectionId}`;
    if (catalogNamesByConn[connectionId] || treeLoadingKeys.includes(key)) {
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
  function loadCatalogSchema(connectionId: string, catalog: string) {
    const key = `${connectionId}::${catalog}`;
    if (treeSchemaCache[key] || treeLoadingKeys.includes(key)) {
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
    setForm({ workspaceId, name: "", driver: "sqlite", sqlitePath: "" });
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

    // Drop this connection's cached tree data so it re-fetches on next expand.
    clearConnectionTreeCache(connection.id);
    queryClient.invalidateQueries({ queryKey: ["database-schema", workspaceId, connection.id] });
    queryClient.invalidateQueries({ queryKey: ["database-catalogs", workspaceId, connection.id] });
  }

  // Remove a single connection's entries from the per-connection tree caches.
  function clearConnectionTreeCache(connectionId: string) {
    const prefix = `${connectionId}::`;
    setCatalogNamesByConn((prev) => {
      if (!(connectionId in prev)) {
        return prev;
      }
      const next = { ...prev };
      delete next[connectionId];
      return next;
    });
    setTreeSchemaCache((prev) => {
      const next: Record<string, DatabaseSchema> = {};
      for (const [key, value] of Object.entries(prev)) {
        if (!key.startsWith(prefix)) {
          next[key] = value;
        }
      }
      return next;
    });
    setTreeErrors((prev) => {
      const next: Record<string, string> = {};
      for (const [key, value] of Object.entries(prev)) {
        if (!key.startsWith(prefix) && key !== `names::${connectionId}`) {
          next[key] = value;
        }
      }
      return next;
    });
  }

  function refreshSelectedSchema() {
    if (!selectedConnection) {
      return;
    }
    refreshConnectionSchema(selectedConnection);
  }

  function selectTable(connectionId: string, table: DatabaseTable) {
    // Selecting a table in another connection makes that connection active so
    // structure, editing, and query context follow the object.
    if (connectionId !== selectedConnectionId) {
      selectConnection(connectionId);
    }
    setSelectedTable(table);
    setClientError(null);
    applyContextFromTable(table);
    layout.setActiveTabId("table");
    layout.setTableSegment("structure");
  }

  // Point the query context at the catalog/schema of a selected object so a
  // query opened from that node runs in the expected place.
  function applyContextFromTable(table: DatabaseTable) {
    setQueryContext({ catalog: table.catalog ?? null, schema: table.schema ?? null });
  }

  function changeQueryContext(patch: { catalog?: string | null; schema?: string | null }) {
    setQueryContext((current) => {
      const next = { ...current, ...patch };
      // Switching catalog invalidates a schema from the previous catalog.
      if (patch.catalog !== undefined && patch.catalog !== current.catalog) {
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
    query?: { orderBy: string | null; orderDescending: boolean; filter: string },
  ) {
    if (connectionId !== selectedConnectionId) {
      selectConnection(connectionId);
    }
    // Switching to a different table drops any prior sort/filter so it does not
    // leak onto an unrelated table; an explicit query (sort/filter action or
    // pagination) takes precedence over that reset.
    const isNewTable = table.name !== selectedTable?.name || connectionId !== selectedConnectionId;
    const effectiveQuery = query ?? (isNewTable ? emptyTableQuery : tableQuery);
    if (!query && isNewTable) {
      setTableQuery(emptyTableQuery);
    }
    setSelectedTable(table);
    setClientError(null);
    applyContextFromTable(table);
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
    if (!selectedConnectionId || !selectedTable) {
      return;
    }
    const current = tableQuery;
    let next: { orderBy: string | null; orderDescending: boolean; filter: string };
    if (current.orderBy !== column) {
      next = { ...current, orderBy: column, orderDescending: false };
    } else if (!current.orderDescending) {
      next = { ...current, orderDescending: true };
    } else {
      next = { ...current, orderBy: null, orderDescending: false };
    }
    setTableQuery(next);
    browseTablePage(
      selectedConnectionId,
      selectedTable,
      0,
      tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE,
      next,
    );
  }

  // Debounce the cross-column filter so typing does not fire a query per key.
  function applyTableFilter(text: string) {
    if (!selectedConnectionId || !selectedTable) {
      return;
    }
    const next = { ...tableQuery, filter: text };
    setTableQuery(next);
    if (filterDebounceRef.current) {
      window.clearTimeout(filterDebounceRef.current);
    }
    const connectionId = selectedConnectionId;
    const table = selectedTable;
    const pageSize = tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE;
    filterDebounceRef.current = window.setTimeout(() => {
      browseTablePage(connectionId, table, 0, pageSize, next);
    }, 350);
  }

  function previewSelectedTable() {
    if (!selectedConnectionId || !selectedTable) {
      return;
    }
    browseTablePage(selectedConnectionId, selectedTable, 0, tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE);
  }

  function refreshTablePage() {
    if (selectedConnectionId && selectedTable && tableView) {
      browseTablePage(selectedConnectionId, selectedTable, tableView.pageIndex, tableView.pageSize);
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
      catalog: selectedTable.catalog,
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
    executeMutation.mutate({
      confirmMutation: pendingSqlConfirmation,
      sql: effectiveSql,
      catalog: queryContext.catalog,
      schema: queryContext.schema,
    });
  }

  function clearSql() {
    setSql("");
    setClientError(null);
    setPendingSqlConfirmation(false);
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
    setPendingSqlConfirmation(false);
    setClientError({ code: "QUERY_CANCELLED", message: t("database.query.cancelled") });
    layout.setResultTab("results");
    if (selectedConnectionId) {
      setConnectionState(selectedConnectionId, {
        message: t("database.query.cancelled"),
        status: "connected",
      });
    }
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
    previewTable: (connectionId: string, table: DatabaseTable) => void;
    refresh: () => void;
    refreshSchema: (connection: DatabaseConnection) => void;
    selectConnection: (connection: DatabaseConnection) => void;
    selectTable: (connectionId: string, table: DatabaseTable) => void;
    toggleCatalog: (connectionId: string, catalog: string) => void;
    toggleConnection: (connection: DatabaseConnection) => void;
    useSql: (sql: string) => void;
  } | null>(null);
  sidebarActionsRef.current = {
    connect: connectConnection,
    delete: setDeleteConfirm,
    disconnect: disconnectConnection,
    edit: handleEditConnection,
    newConnection: handleNewConnection,
    newQuery: startNewQuery,
    previewTable: (connectionId, table) =>
      browseTablePage(connectionId, table, 0, tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE),
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
      onDeleteConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.delete(connection),
      onDisconnect: (connection: DatabaseConnection) => sidebarActionsRef.current?.disconnect(connection),
      onEditConnection: (connection: DatabaseConnection) => sidebarActionsRef.current?.edit(connection),
      onNewConnection: () => sidebarActionsRef.current?.newConnection(),
      onNewQuery: () => sidebarActionsRef.current?.newQuery(),
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
      onUseSql: (sql: string) => sidebarActionsRef.current?.useSql(sql),
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

  const activeError = clientError ?? (layout.activeTabId === "table" ? browseMutation.error : executeMutation.error);
  const executePending = executeMutation.isPending || browseMutation.isPending;

  // Inline editing is available when a real table with a primary key is being
  // browsed on a connected session; the primary key locates rows for the
  // update/delete row commands.
  const primaryKeyColumns = (selectedTable?.columns ?? [])
    .filter((column) => column.primaryKey)
    .map((column) => column.name);
  const tableEditing: TableEditing | null =
    selectedTable &&
    tableView &&
    selectedConnectionStatus === "connected" &&
    primaryKeyColumns.length > 0 &&
    !selectedConnection?.readOnly
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
        onStop={stopQuery}
        pendingConfirmation={pendingSqlConfirmation}
        selectedConnectionId={selectedConnectionId}
        sqlDirty={sql.trim().length > 0}
      />
      <div className="flex min-h-0 flex-1 flex-col">
        <DatabaseWorkspace
          activeResultTab={layout.resultTab}
          activeStructureTab={layout.inspectorTab}
          activeTabId={layout.activeTabId}
          connections={connections}
          error={activeError}
          executePending={executePending}
          history={queryHistory}
          catalogOptions={catalogOptions}
          onChangeQueryContext={changeQueryContext}
          onClearSql={clearSql}
          onClearHistory={clearQueryHistory}
          onPreviewSelectedTable={previewSelectedTable}
          onRefreshSchema={refreshSelectedSchema}
          onRun={runSql}
          onSelectConnection={(connectionId) => selectConnection(connectionId || null)}
          queryCatalog={queryContext.catalog}
          querySchema={queryContext.schema}
          schemaOptions={schemaOptions}
          onSelectHistory={loadHistoryEntry}
          onSelectResultTab={layout.setResultTab}
          onSelectStructureTab={layout.setInspectorTab}
          onSelectTab={layout.setActiveTabId}
          onSelectTableSegment={layout.setTableSegment}
          onShowHistory={showQueryHistory}
          onSqlChange={setSql}
          onStop={stopQuery}
          onTableFilter={applyTableFilter}
          onTablePageChange={(pageIndex, pageSize) =>
            selectedConnectionId &&
            selectedTable &&
            browseTablePage(selectedConnectionId, selectedTable, pageIndex, pageSize)
          }
          onTableSort={applyTableSort}
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
          tableFilter={tableQuery.filter}
          tableSegment={layout.tableSegment}
          tableSort={
            tableQuery.orderBy
              ? { column: tableQuery.orderBy, descending: tableQuery.orderDescending }
              : null
          }
          tableView={tableView}
          workspaceId={workspaceId}
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
