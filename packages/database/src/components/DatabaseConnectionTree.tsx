import { Columns3, Copy, Database, Eye, Pencil, Play, PlusCircle, RefreshCw, Square, Table2, Trash2 } from "lucide-react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable } from "@unfour/command-client";
import {
  Badge,
  ConnectionStatus,
  ContextMenuItem,
  EmptyState,
  IconButton,
  TreeView,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import { buildDatabaseTree, databaseTableTreeId } from "../model/database-tree";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";

export function DatabaseConnectionTree({
  catalogNamesByConnection,
  connectionStates,
  connections,
  loadingKeys,
  loadErrors,
  onConnect,
  onDesignTable,
  onDeleteConnection,
  onDisconnect,
  onEditConnection,
  onNewQuery,
  onPreviewTable,
  onRefreshSchema,
  onSelectConnection,
  onSelectTable,
  onToggleCatalog,
  onToggleConnection,
  onUseSql,
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
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onSelectConnection: (connection: DatabaseConnection) => void;
  onSelectTable?: (connectionId: string, table: DatabaseTable) => void;
  /** Fired when a database node is expanded, so its schema can be lazy-loaded. */
  onToggleCatalog?: (connectionId: string, catalog: string) => void;
  /** Fired when a connection node is expanded, so its databases can load. */
  onToggleConnection?: (connection: DatabaseConnection) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
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
  // id to {connectionId, catalog}. Used to route selection and lazy loading.
  const tableLookup = new Map<string, { connectionId: string; table: DatabaseTable }>();
  const catalogLookup = new Map<string, { connectionId: string; catalog: string }>();
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
          onEditConnection={onEditConnection}
          onNewQuery={onNewQuery}
          onRefreshSchema={onRefreshSchema}
          status={status}
        />
      ),
      children:
        status === "connected"
          ? buildConnectionChildren({
              catalogLookup,
              catalogNames: catalogNamesByConnection?.[connection.id],
              connection,
              defaultExpandedIds,
              loadErrors,
              loadingKeys,
              onDesignTable,
              onPreviewTable,
              onRefreshSchema,
              onUseSql,
              schemaCache,
              status,
              t,
              tableLookup,
            })
          : undefined,
      icon: <Database size={13} />,
      id: connection.id,
      label: connection.name,
      loading: rootLoading,
      meta: (
        <ConnectionStatus
          dotOnly
          label={statusLabel}
          status={status === "failed" ? "error" : status}
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
  loadErrors,
  loadingKeys,
  onDesignTable,
  onPreviewTable,
  onRefreshSchema,
  onUseSql,
  schemaCache,
  status,
  t,
  tableLookup,
}: {
  catalogLookup: Map<string, { connectionId: string; catalog: string }>;
  catalogNames?: string[];
  connection: DatabaseConnection;
  defaultExpandedIds: Set<string>;
  loadErrors?: Record<string, string>;
  loadingKeys?: string[];
  onDesignTable?: (connectionId: string, table: DatabaseTable) => void;
  onPreviewTable?: (connectionId: string, table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onUseSql?: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  schemaCache?: Record<string, DatabaseSchema>;
  status: DatabaseConnectionStatus;
  t: ReturnType<typeof useI18n>["t"];
  tableLookup: Map<string, { connectionId: string; table: DatabaseTable }>;
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
    return [
      {
        disabled: true,
        id: `${connection.id}:failed`,
        label: t("database.tree.connectionFailed"),
      },
    ];
  }

  const isLoading = (key: string) => loadingKeys?.includes(key) ?? false;
  const errorOf = (key: string) => loadErrors?.[key];

  // SQLite: a single file with no catalog level. Its objects load under the
  // connection node directly (catalog key "").
  if (connection.driver === "sqlite") {
    const key = `${connection.id}::`;
    const schema = schemaCache?.[key];
    if (schema) {
      return renderCatalogContents({
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

  return catalogNames.map((name) => {
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
      {onUseSql && (
        <ContextMenuItem onSelect={() => onUseSql(connection.id, generateInsertSql(connection.driver, table), table)}>
          {t("database.tree.generateInsert")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void navigator.clipboard?.writeText(table.name)}>
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
  onEditConnection,
  onNewQuery,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: (connection?: DatabaseConnection) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();

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
      <ContextMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.name)}>
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
