import { Columns3, Copy, CopyPlus, Database, Eye, FileText, MoreVertical, Pencil, Play, PlusCircle, RefreshCw, Square, Table2, Trash2 } from "lucide-react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable, SavedSql } from "@unfour/command-client";
import {
  Badge,
  ConnectionStatus,
  ContextMenuItem,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  IconButton,
  StatusBadge,
  TreeView,
  useFeedbackErrorHandler,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import { buildDatabaseTree, databaseTableTreeId } from "../model/database-tree";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";

async function copyToClipboard(value: string, onError: (error: unknown) => void) {
  try {
    if (!navigator.clipboard) {
      throw new Error("Clipboard API is unavailable in this context");
    }
    await navigator.clipboard.writeText(value);
  } catch (error) {
    onError(error);
  }
}

export function DatabaseConnectionTree({
  catalogNamesByConnection,
  connectionStates,
  connections,
  loadingKeys,
  loadErrors,
  onConnect,
  onDeleteConnection,
  onDeleteSavedSql,
  onDesignTable,
  onDisconnect,
  onDuplicateConnection,
  onEditConnection,
  onNewQuery,
  onOpenSavedSql,
  onPreviewTable,
  onRefreshSchema,
  onSelectConnection,
  onSelectTable,
  onToggleCatalog,
  onToggleConnection,
  onUseSql,
  savedSqlByConnection,
  schemaCache,
  selectedConnectionId,
  selectedTableId,
}: {
  /** Server databases per connection (PostgreSQL/MySQL). Loaded on expand. */
  catalogNamesByConnection?: Record<string, string[]>;
  connectionStates?: Record<string, DatabaseConnectionSessionState>;
  connections: DatabaseConnection[];
  /** Keys (`names::id` / `id::catalog`) whose fetch is in flight. */
  loadingKeys?: string[];
  /** Error messages keyed the same way, surfaced inline in the tree. */
  loadErrors?: Record<string, string>;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDeleteSavedSql?: (item: SavedSql) => void;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onDuplicateConnection?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onOpenSavedSql?: (item: SavedSql) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onSelectConnection: (connection: DatabaseConnection) => void;
  onSelectTable?: (connectionId: string, table: DatabaseTable) => void;
  /** Fired when a database node is expanded, so its schema can be lazy-loaded. */
  onToggleCatalog?: (connectionId: string, catalog: string) => void;
  /** Fired when a connection node is expanded, so its databases can load. */
  onToggleConnection?: (connection: DatabaseConnection) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  /** Saved SQL snippets grouped by their owning connection id. */
  savedSqlByConnection?: Record<string, SavedSql[]>;
  /** Loaded schemas keyed `${connectionId}::${catalog}` (catalog "" for SQLite). */
  schemaCache?: Record<string, DatabaseSchema>;
  selectedConnectionId: string | null;
  selectedTableId?: string | null;
}) {
  const { t } = useI18n();

  if (!connections.length) {
    return <EmptyState className="min-h-[72px]">{t("database.errors.noConnections")}</EmptyState>;
  }

  // Maps a table node id to the table and its owning connection; a catalog node
  // id to {connectionId, catalog}; a saved-sql node id to its SavedSql record.
  // Used to route selection and lazy loading.
  const tableLookup = new Map<string, { connectionId: string; table: DatabaseTable }>();
  const catalogLookup = new Map<string, { connectionId: string; catalog: string }>();
  const savedSqlLookup = new Map<string, SavedSql>();
  const defaultExpandedIds = new Set<string>();
  const selectedConnection = connections.find((connection) => connection.id === selectedConnectionId) ?? null;

  const items: TreeViewItem[] = connections.map((connection) => {
    const selected = connection.id === selectedConnectionId;
    const session = connectionStates?.[connection.id];
    const status = resolveConnectionStatus({ session });
    const statusLabel = databaseConnectionStatusLabel(status, t);
    const rootLoading =
      status === "connected" &&
      Boolean(
        loadingKeys?.some((key) => key === `names::${connection.id}` || key.startsWith(`${connection.id}::`)),
      );

    // Auto-expand only once the connection has succeeded, so saved but unopened
    // connections stay as plain rows until they can actually reveal schema data.
    if (selected && status === "connected") {
      defaultExpandedIds.add(connection.id);
    }

    return {
      contextMenu: (
        <ConnectionContextMenu
          connection={connection}
          onConnect={onConnect}
          onDeleteConnection={onDeleteConnection}
          onDisconnect={onDisconnect}
          onDuplicateConnection={onDuplicateConnection}
          onEditConnection={onEditConnection}
          onNewQuery={onNewQuery}
          onRefreshSchema={onRefreshSchema}
          status={status}
        />
      ),
      // Right-aligned "⋯" menu on each row, mirroring the right-click menu so
      // actions (including delete) are reachable without opening the dialog.
      // The button reveals on row hover (standard explorer pattern): it stays
      // mounted with reserved width to avoid layout shift, but is only painted
      // when the row is hovered or keyboard-focused, which removes the always-on
      // hover/tooltip repaint that made a single row flicker.
      actions: (
        <span className="opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100">
          <ConnectionRowMenu
            connection={connection}
            onConnect={onConnect}
            onDeleteConnection={onDeleteConnection}
            onDisconnect={onDisconnect}
            onDuplicateConnection={onDuplicateConnection}
            onEditConnection={onEditConnection}
            onNewQuery={onNewQuery}
            onRefreshSchema={onRefreshSchema}
            status={status}
          />
        </span>
      ),
      children:
        status === "connected"
          ? buildConnectionChildren({
              catalogLookup,
              catalogNames: catalogNamesByConnection?.[connection.id],
              connection,
              defaultExpandedIds,
              failureMessage: session?.message,
              loadErrors,
              loadingKeys,
              onDeleteSavedSql,
              onDesignTable,
              onOpenSavedSql,
              onPreviewTable,
              onRefreshSchema,
              onUseSql,
              savedSql: savedSqlByConnection?.[connection.id],
              schemaCache,
              status,
              t,
              tableLookup,
              savedSqlLookup,
            })
          : undefined,
      icon: <Database size={13} />,
      id: connection.id,
      label: connection.name,
      loading: rootLoading,
      meta:
        status === "failed" ? (
          <StatusBadge tone="danger">{statusLabel}</StatusBadge>
        ) : (
          <ConnectionStatus
            dotOnly
            label={statusLabel}
            status={status}
            variant="dot"
          />
        ),
      title: connectionStateTitle(connection, session),
    };
  });

  const selectedId = selectedTableId ?? selectedConnection?.id ?? null;

  return (
    <TreeView
      // Remount only when the set of connections changes, so expanding one
      // connection (or database) never collapses the others.
      key={connections.map((connection) => connection.id).join(",")}
      defaultExpandedIds={[...defaultExpandedIds]}
      items={items}
      onActivate={(item) => {
        // Double-click a saved SQL snippet -> open it in a query tab.
        const savedSql = savedSqlLookup.get(item.id);
        if (savedSql) {
          onOpenSavedSql?.(savedSql);
          return;
        }
        // Double-click a table -> preview data (Navicat convention)
        const tableEntry = tableLookup.get(item.id);
        if (tableEntry) {
          onPreviewTable?.(tableEntry.connectionId, tableEntry.table);
          return;
        }
        // Double-click a connection -> connect (Navicat convention)
        const connection = connections.find((candidate) => candidate.id === item.id);
        if (connection) {
          onConnect?.(connection);
        }
      }}
      onSelect={(item) => {
        // Single click a saved SQL snippet -> select only (no auto-open).
        if (savedSqlLookup.has(item.id)) {
          return;
        }
        // Single click a table -> select only (lightweight highlight, no Tab switch)
        const entry = tableLookup.get(item.id);
        if (entry) {
          onSelectTable?.(entry.connectionId, entry.table);
          return;
        }

        const connection = connections.find((candidate) => candidate.id === item.id);
        if (connection) {
          onSelectConnection(connection);
        }
      }}
      onToggle={(id, expanded) => {
        if (!expanded) {
          return;
        }
        const catalog = catalogLookup.get(id);
        if (catalog) {
          onToggleCatalog?.(catalog.connectionId, catalog.catalog);
          return;
        }
        const connection = connections.find((candidate) => candidate.id === id);
        if (connection) {
          onToggleConnection?.(connection);
        }
      }}
      selectedId={selectedId}
    />
  );
}

function buildConnectionChildren({
  catalogLookup,
  catalogNames,
  connection,
  defaultExpandedIds,
  failureMessage,
  loadErrors,
  loadingKeys,
  onDeleteSavedSql,
  onDesignTable,
  onOpenSavedSql,
  onPreviewTable,
  onRefreshSchema,
  onUseSql,
  savedSql,
  schemaCache,
  status,
  t,
  tableLookup,
  savedSqlLookup,
}: {
  catalogLookup: Map<string, { connectionId: string; catalog: string }>;
  catalogNames?: string[];
  connection: DatabaseConnection;
  defaultExpandedIds: Set<string>;
  /** The real failure reason from the connection session, shown in the tree when status is `failed`. */
  failureMessage?: string | null;
  loadErrors?: Record<string, string>;
  loadingKeys?: string[];
  onDeleteSavedSql?: (item: SavedSql) => void;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onOpenSavedSql?: (item: SavedSql) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  savedSql?: SavedSql[];
  schemaCache?: Record<string, DatabaseSchema>;
  status: DatabaseConnectionStatus;
  t: ReturnType<typeof useI18n>["t"];
  tableLookup: Map<string, { connectionId: string; table: DatabaseTable }>;
  savedSqlLookup: Map<string, SavedSql>;
}): TreeViewItem[] | undefined {
  if (status === "disconnected") {
    return [
      {
        disabled: true,
        id: `${connection.id}:disconnected`,
        label: t("database.tree.connectToBrowse"),
      },
    ];
  }

  if (status === "failed") {
    const failureLabel = failureMessage ?? t("database.tree.connectionFailed");
    return [
      {
        disabled: true,
        id: `${connection.id}:failed`,
        label: failureLabel,
        title: failureLabel,
      },
    ];
  }

  const isLoading = (key: string) => loadingKeys?.includes(key) ?? false;
  const errorOf = (key: string) => loadErrors?.[key];

  // Saved SQL snippets are a connection-level asset (not per-catalog), so the
  // group sits beside the catalog list / schema contents. Built once and
  // appended to whichever children array the driver path produces.
  const savedSqlGroup = buildSavedSqlGroup({
    connection,
    onDeleteSavedSql,
    onOpenSavedSql,
    savedSql,
    savedSqlLookup,
    t,
  });

  // SQLite: a single file with no catalog level. Its objects load under the
  // connection node directly (catalog key "").
  if (connection.driver === "sqlite") {
    const key = `${connection.id}::`;
    const schema = schemaCache?.[key];
    if (schema) {
      const contents = renderCatalogContents({
        connection,
        defaultExpandedIds,
        onDesignTable,
        onPreviewTable,
        onRefreshSchema,
        onUseSql,
        parentId: connection.id,
        t,
        tableLookup,
        tables: schema.tables,
      });
      return savedSqlGroup ? [...contents, savedSqlGroup] : contents;
    }
    return [statusChild(key, isLoading(key), errorOf(key), t)];
  }

  // PostgreSQL / MySQL: one node per server database, each loaded on expand.
  const namesKey = `names::${connection.id}`;
  if (!catalogNames) {
    return [statusChild(namesKey, isLoading(namesKey), errorOf(namesKey), t)];
  }
  if (!catalogNames.length) {
    return [
      {
        disabled: true,
        id: `${connection.id}:no-databases`,
        label: t("database.tree.noDatabases"),
      },
    ];
  }

  const connectedCatalog = connection.database?.trim() || null;

  const catalogNodes = catalogNames.map((name) => {
    const catalogNodeId = `${connection.id}:catalog:${name}`;
    catalogLookup.set(catalogNodeId, { connectionId: connection.id, catalog: name });
    const key = `${connection.id}::${name}`;
    const schema = schemaCache?.[key];

    let children: TreeViewItem[];
    if (schema) {
      children = renderCatalogContents({
        connection,
        defaultExpandedIds,
        onDesignTable,
        onPreviewTable,
        onRefreshSchema,
        onUseSql,
        parentId: catalogNodeId,
        t,
        tableLookup,
        tables: schema.tables,
      });
      // Auto-expand the connected database so it shows useful content as soon
      // as it loads; other databases stay collapsed until opened.
      if (name === connectedCatalog) {
        defaultExpandedIds.add(catalogNodeId);
      }
    } else {
      children = [statusChild(key, isLoading(key), errorOf(key), t)];
    }

    return {
      children,
      icon: <Database size={13} />,
      id: catalogNodeId,
      label: name,
      loading: isLoading(key),
      title: name,
    };
  });

  return savedSqlGroup ? [...catalogNodes, savedSqlGroup] : catalogNodes;
}

// Build the "Saved Queries" group node shown beside the catalog/schema tree.
// Returns null when no callback is wired (the host page does not support
// opening saved SQL) so the tree simply omits the group instead of showing a
// dead branch.
function buildSavedSqlGroup({
  connection,
  onDeleteSavedSql,
  onOpenSavedSql,
  savedSql,
  savedSqlLookup,
  t,
}: {
  connection: DatabaseConnection;
  onDeleteSavedSql?: (item: SavedSql) => void;
  onOpenSavedSql?: (item: SavedSql) => void;
  savedSql?: SavedSql[];
  savedSqlLookup: Map<string, SavedSql>;
  t: ReturnType<typeof useI18n>["t"];
}): TreeViewItem | null {
  if (!onOpenSavedSql && !onDeleteSavedSql) {
    return null;
  }
  const items = savedSql ?? [];
  const groupId = `${connection.id}:saved-sql`;
  const children = items.length
    ? items.map((item) => {
        const id = `${connection.id}:saved-sql:${item.id}`;
        savedSqlLookup.set(id, item);
        return {
          contextMenu: (
            <SavedSqlContextMenu
              item={item}
              onDelete={onDeleteSavedSql}
              onOpen={onOpenSavedSql}
              t={t}
            />
          ),
          icon: <FileText size={13} />,
          id,
          label: item.name,
          title: item.sql,
        };
      })
    : [
        {
          disabled: true,
          id: `${groupId}:empty`,
          label: t("database.saved.empty"),
        },
      ];

  return {
    children,
    icon: <FileText size={13} />,
    id: groupId,
    label: t("database.tree.savedQueriesGroup"),
    meta: <Badge tone="neutral">{items.length}</Badge>,
  };
}

function SavedSqlContextMenu({
  item,
  onDelete,
  onOpen,
  t,
}: {
  item: SavedSql;
  onDelete?: (item: SavedSql) => void;
  onOpen?: (item: SavedSql) => void;
  t: ReturnType<typeof useI18n>["t"];
}) {
  const handleError = useFeedbackErrorHandler();
  return (
    <>
      {onOpen && (
        <ContextMenuItem onSelect={() => onOpen(item)}>
          <Play size={13} />
          {t("database.tree.openSavedSql")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void copyToClipboard(item.sql, handleError)}>
        <Copy size={13} />
        {t("database.tree.copySavedSql")}
      </ContextMenuItem>
      {onDelete && (
        <ContextMenuItem onSelect={() => onDelete(item)} tone="danger">
          <Trash2 size={13} />
          {t("database.tree.deleteSavedSql")}
        </ContextMenuItem>
      )}
    </>
  );
}

// A disabled child reflecting the load state of a lazily-fetched node: an error
// message when the fetch failed, a spinner label while in flight, otherwise an
// "expand to load" hint.
function statusChild(
  key: string,
  loading: boolean,
  error: string | undefined,
  t: ReturnType<typeof useI18n>["t"],
): TreeViewItem {
  if (error) {
    return { disabled: true, id: `${key}:error`, label: error, title: error };
  }
  return {
    disabled: true,
    id: `${key}:${loading ? "loading" : "placeholder"}`,
    label: loading ? t("database.tree.loadingSchema") : t("database.tree.expandToLoad"),
  };
}

// Render the contents of a single catalog (database): PostgreSQL nests schemas,
// while MySQL/SQLite list table groups directly. Shared by the lazy multi-catalog
// renderer and the single-datasource (SQLite) renderer.
function renderCatalogContents({
  connection,
  defaultExpandedIds,
  onDesignTable,
  onPreviewTable,
  onRefreshSchema,
  onUseSql,
  parentId,
  t,
  tableLookup,
  tables,
}: {
  connection: DatabaseConnection;
  defaultExpandedIds: Set<string>;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  parentId: string;
  t: ReturnType<typeof useI18n>["t"];
  tableLookup: Map<string, { connectionId: string; table: DatabaseTable }>;
  tables: DatabaseTable[];
}): TreeViewItem[] {
  if (!tables.length) {
    return [
      {
        disabled: true,
        id: `${parentId}:no-tables`,
        label: "No tables or views found",
      },
    ];
  }

  const catalog = buildDatabaseTree(tables).catalogs[0];
  if (!catalog || !catalog.hasSchemaLevel) {
    return buildTableGroups({
      connection,
      defaultExpandedIds,
      onDesignTable,
      onPreviewTable,
      onUseSql,
      parentId,
      t,
      tableLookup,
      tables,
    });
  }

  return catalog.schemas.map((schemaNode) => {
    const schemaNodeId = `${parentId}:schema:${schemaNode.key}`;
    defaultExpandedIds.add(schemaNodeId);
    return {
      actions: onRefreshSchema ? (
        <IconButton label={`Refresh ${schemaNode.key} schema`} onClick={() => onRefreshSchema(connection)} size="compact">
          <RefreshCw size={12} />
        </IconButton>
      ) : undefined,
      children: buildTableGroups({
        connection,
        defaultExpandedIds,
        onDesignTable,
        onPreviewTable,
        onUseSql,
        parentId: schemaNodeId,
        t,
        tableLookup,
        tables: schemaNode.tables,
      }),
      icon: <Columns3 size={13} />,
      id: schemaNodeId,
      label: schemaNode.key,
      meta: <Badge tone="neutral">{schemaNode.tables.length}</Badge>,
      title: schemaNode.key,
    };
  });
}

// Categorize a schema's objects into Tables and Views group nodes (each with a
// count badge), matching the asset-tree hierarchy in the design mockup. The
// caller has already guarded against the empty-schema case.
function buildTableGroups({
  connection,
  defaultExpandedIds,
  onDesignTable,
  onPreviewTable,
  onUseSql,
  parentId,
  t,
  tableLookup,
  tables,
}: {
  connection: DatabaseConnection;
  defaultExpandedIds: Set<string>;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  parentId: string;
  t: ReturnType<typeof useI18n>["t"];
  tableLookup: Map<string, { connectionId: string; table: DatabaseTable }>;
  tables: DatabaseTable[];
}): TreeViewItem[] {
  const baseTables = tables.filter((table) => !isViewKind(table.kind));
  const views = tables.filter((table) => isViewKind(table.kind));
  const groups: TreeViewItem[] = [];

  if (baseTables.length) {
    const groupId = `${parentId}:tables`;
    defaultExpandedIds.add(groupId);
    groups.push({
      children: baseTables.map((table) =>
        tableItem({ connection, onDesignTable, onPreviewTable, onUseSql, t, table, tableLookup }),
      ),
      icon: <Table2 size={13} />,
      id: groupId,
      label: t("database.tree.tablesGroup"),
      meta: <Badge tone="teal">{baseTables.length}</Badge>,
    });
  }

  if (views.length) {
    const groupId = `${parentId}:views`;
    groups.push({
      children: views.map((table) =>
        tableItem({ connection, onDesignTable, onPreviewTable, onUseSql, t, table, tableLookup }),
      ),
      icon: <Eye size={13} />,
      id: groupId,
      label: t("database.tree.viewsGroup"),
      meta: <Badge tone="neutral">{views.length}</Badge>,
    });
  }

  return groups;
}

function isViewKind(kind: string) {
  return kind.toLowerCase().includes("view");
}

function tableItem({
  connection,
  onDesignTable,
  onPreviewTable,
  onUseSql,
  t,
  table,
  tableLookup,
}: {
  connection: DatabaseConnection;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  t: ReturnType<typeof useI18n>["t"];
  table: DatabaseTable;
  tableLookup: Map<string, { connectionId: string; table: DatabaseTable }>;
}): TreeViewItem {
  const id = databaseTableTreeId(connection.id, table);
  tableLookup.set(id, { connectionId: connection.id, table });
  return {
    contextMenu: (
      <TableContextMenu
        connection={connection}
        onDesignTable={onDesignTable}
        onPreviewTable={onPreviewTable}
        onUseSql={onUseSql}
        t={t}
        table={table}
      />
    ),
    icon: <Table2 size={13} />,
    id,
    label: table.name,
    title: [table.catalog, table.schema, table.name].filter(Boolean).join("."),
  };
}

function TableContextMenu({
  connection,
  onDesignTable,
  onPreviewTable,
  onUseSql,
  t,
  table,
}: {
  connection: DatabaseConnection;
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  t: ReturnType<typeof useI18n>["t"];
  table: DatabaseTable;
}) {
  const handleError = useFeedbackErrorHandler();
  return (
    <>
      {onPreviewTable && (
        <ContextMenuItem onSelect={() => onPreviewTable(connection.id, table)}>
          {t("database.tree.previewData")}
        </ContextMenuItem>
      )}
      {onDesignTable && (
        <ContextMenuItem onSelect={() => onDesignTable(connection.id, table)}>
          {t("database.tree.designTable")}
        </ContextMenuItem>
      )}
      {onUseSql && (
        <ContextMenuItem onSelect={() => onUseSql(connection.id, generateSelectSql(connection.driver, table), table)}>
          {t("database.tree.generateSelect")}
        </ContextMenuItem>
      )}
      {/* Views are read-only: hide "Generate INSERT" to avoid offering an
          action that will fail at execution time. Tables keep the full menu. */}
      {onUseSql && table.kind !== "view" && (
        <ContextMenuItem onSelect={() => onUseSql(connection.id, generateInsertSql(connection.driver, table), table)}>
          {t("database.tree.generateInsert")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void copyToClipboard(table.name, handleError)}>
        {t("database.tree.copyTableName")}
      </ContextMenuItem>
    </>
  );
}

function quoteDbIdentifier(driver: string, value: string) {
  if (driver === "mysql") {
    return `\`${value.split("`").join("``")}\``;
  }
  return `"${value.split('"').join('""')}"`;
}

function qualifiedSqlName(driver: string, table: DatabaseTable) {
  const name = quoteDbIdentifier(driver, table.name);
  // PostgreSQL qualifies by schema; MySQL qualifies by its database (catalog).
  const qualifier = table.schema ?? table.catalog;
  return qualifier ? `${quoteDbIdentifier(driver, qualifier)}.${name}` : name;
}

function generateSelectSql(driver: string, table: DatabaseTable) {
  const columns = table.columns.length
    ? table.columns.map((column) => quoteDbIdentifier(driver, column.name)).join(", ")
    : "*";
  return `SELECT ${columns}\nFROM ${qualifiedSqlName(driver, table)}\nLIMIT 100;`;
}

function generateInsertSql(driver: string, table: DatabaseTable) {
  if (!table.columns.length) {
    return `INSERT INTO ${qualifiedSqlName(driver, table)} () VALUES ();`;
  }
  const columns = table.columns.map((column) => quoteDbIdentifier(driver, column.name)).join(", ");
  const placeholders = table.columns.map(() => "NULL").join(", ");
  return `INSERT INTO ${qualifiedSqlName(driver, table)} (${columns})\nVALUES (${placeholders});`;
}

function ConnectionContextMenu({
  connection,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onDuplicateConnection,
  onEditConnection,
  onNewQuery,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onDuplicateConnection?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();

  return (
    <>
      <ContextMenuItem onSelect={() => onConnect?.(connection)}>
        <Play size={13} />
        {t("common.actions.connect")}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={status === "disconnected"}
        onSelect={() => onDisconnect?.(connection)}
      >
        <Square size={13} />
        {t("common.actions.disconnect")}
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => onNewQuery?.(connection)}>
        <PlusCircle size={13} />
        {t("database.actions.newQuery")}
      </ContextMenuItem>
      <ContextMenuItem onSelect={() => onRefreshSchema?.(connection)}>
        <RefreshCw size={13} />
        {t("database.actions.refreshSchema")}
      </ContextMenuItem>
      {onEditConnection && (
        <ContextMenuItem onSelect={() => onEditConnection(connection)}>
          <Pencil size={13} />
          {t("database.tree.editConnection")}
        </ContextMenuItem>
      )}
      {onDuplicateConnection && (
        <ContextMenuItem onSelect={() => onDuplicateConnection(connection)}>
          <CopyPlus size={13} />
          {t("database.tree.duplicateConnection")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void copyToClipboard(connection.name, handleError)}>
        <Copy size={13} />
        {t("database.tree.copyName")}
      </ContextMenuItem>
      {onDeleteConnection && (
        <ContextMenuItem onSelect={() => onDeleteConnection(connection)} tone="danger">
          <Trash2 size={13} />
          {t("database.tree.deleteConnection")}
        </ContextMenuItem>
      )}
    </>
  );
}

// Right-aligned row menu (the "⋯" button) — kept in sync with
// ConnectionContextMenu so the inline menu and the right-click menu expose the
// same actions. Item order matches the context menu for consistency.
function ConnectionRowMenu({
  connection,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onDuplicateConnection,
  onEditConnection,
  onNewQuery,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onDuplicateConnection?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton
          disableTooltip
          label={t("database.tree.actionsLabel", { name: connection.name })}
          size="compact"
        >
          <MoreVertical size={14} />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuItem onSelect={() => onConnect?.(connection)}>
          <Play size={13} />
          {t("common.actions.connect")}
        </DropdownMenuItem>
        <DropdownMenuItem disabled={status === "disconnected"} onSelect={() => onDisconnect?.(connection)}>
          <Square size={13} />
          {t("common.actions.disconnect")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onNewQuery?.(connection)}>
          <PlusCircle size={13} />
          {t("database.actions.newQuery")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onRefreshSchema?.(connection)}>
          <RefreshCw size={13} />
          {t("database.actions.refreshSchema")}
        </DropdownMenuItem>
        {onEditConnection && (
          <DropdownMenuItem onSelect={() => onEditConnection(connection)}>
            <Pencil size={13} />
            {t("database.tree.editConnection")}
          </DropdownMenuItem>
        )}
        {onDuplicateConnection && (
          <DropdownMenuItem onSelect={() => onDuplicateConnection(connection)}>
            <CopyPlus size={13} />
            {t("database.tree.duplicateConnection")}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={() => void copyToClipboard(connection.name, handleError)}>
          <Copy size={13} />
          {t("database.tree.copyName")}
        </DropdownMenuItem>
        {onDeleteConnection && (
          <DropdownMenuItem
            className="text-[var(--u-color-danger)] data-[highlighted]:bg-[var(--u-color-danger-soft)]"
            onSelect={() => onDeleteConnection(connection)}
          >
            <Trash2 size={13} />
            {t("database.tree.deleteConnection")}
          </DropdownMenuItem>
        )}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

function resolveConnectionStatus({
  session,
}: {
  session?: DatabaseConnectionSessionState;
}): DatabaseConnectionStatus {
  return session?.status ?? "disconnected";
}

function databaseConnectionStatusLabel(
  status: DatabaseConnectionStatus,
  t: ReturnType<typeof useI18n>["t"],
) {
  if (status === "connecting") {
    return t("common.actions.connecting");
  }
  return t(`database.connection.${status}`);
}

function connectionStateTitle(
  connection: DatabaseConnection,
  session?: DatabaseConnectionSessionState,
) {
  const message = session?.message ? ` - ${session.message}` : "";
  return `${connection.name} (${connection.driver})${message}`;
}
