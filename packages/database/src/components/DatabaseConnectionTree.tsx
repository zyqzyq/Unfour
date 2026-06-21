import { Columns3, Copy, Database, MoreHorizontal, Pencil, Play, RefreshCw, Table2, Trash2 } from "lucide-react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable } from "@unfour/command-client";
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
  TreeView,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import { databaseTableTreeId } from "../model/database-tree";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";

export function DatabaseConnectionTree({
  connectionStates,
  connections,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onEditConnection,
  onNewQuery,
  onPreviewTable,
  onRefresh,
  onRefreshSchema,
  onSelectConnection,
  onSelectTable,
  onUseSql,
  schema,
  schemaLoading = false,
  selectedConnectionId,
  selectedTableId,
}: {
  connectionStates?: Record<string, DatabaseConnectionSessionState>;
  connections: DatabaseConnection[];
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: () => void;
  onPreviewTable?: (table: DatabaseTable) => void;
  onRefresh?: () => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onSelectConnection: (connection: DatabaseConnection) => void;
  onSelectTable?: (table: DatabaseTable) => void;
  onUseSql?: (sql: string) => void;
  schema?: DatabaseSchema;
  schemaLoading?: boolean;
  selectedConnectionId: string | null;
  selectedTableId?: string | null;
}) {
  const { t } = useI18n();

  if (!connections.length) {
    return <EmptyState className="min-h-[72px]">{t("database.errors.noConnections")}</EmptyState>;
  }

  const tableLookup = new Map<string, DatabaseTable>();
  const defaultExpandedIds = new Set<string>();
  const selectedConnection = connections.find((connection) => connection.id === selectedConnectionId) ?? null;

  const items: TreeViewItem[] = connections.map((connection) => {
    const selected = connection.id === selectedConnectionId;
    const session = connectionStates?.[connection.id];
    const status = resolveConnectionStatus({
      hasSchema: selected && Boolean(schema),
      selected,
      session,
    });

    if (selected) {
      defaultExpandedIds.add(connection.id);
    }

    return {
      actions: (
        <ConnectionActions
          connection={connection}
          onConnect={onConnect}
          onDeleteConnection={onDeleteConnection}
          onDisconnect={onDisconnect}
          onEditConnection={onEditConnection}
          onNewQuery={onNewQuery}
          onRefresh={onRefresh}
          onRefreshSchema={onRefreshSchema}
          status={status}
        />
      ),
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
      children: selected
        ? buildSelectedConnectionChildren({
            connection,
            defaultExpandedIds,
            onPreviewTable,
            onRefreshSchema,
            onUseSql,
            schema,
            schemaLoading,
            status,
            t,
            tableLookup,
          })
        : undefined,
      icon: <Database size={13} />,
      id: connection.id,
      label: connection.name,
      meta: (
        <ConnectionStatus
          label={status}
          status={status === "failed" ? "error" : status}
        />
      ),
      title: connectionStateTitle(connection, session),
    };
  });

  const selectedId = selectedTableId ?? selectedConnection?.id ?? null;

  return (
    <TreeView
      key={[selectedConnectionId, schema?.tables.length ?? 0, schemaLoading ? "loading" : "idle"].join(":")}
      defaultExpandedIds={[...defaultExpandedIds]}
      items={items}
      onSelect={(item) => {
        const table = tableLookup.get(item.id);
        if (table) {
          onSelectTable?.(table);
          return;
        }

        const connection = connections.find((candidate) => candidate.id === item.id);
        if (connection) {
          onSelectConnection(connection);
        }
      }}
      selectedId={selectedId}
    />
  );
}

function buildSelectedConnectionChildren({
  connection,
  defaultExpandedIds,
  onPreviewTable,
  onRefreshSchema,
  onUseSql,
  schema,
  schemaLoading,
  status,
  t,
  tableLookup,
}: {
  connection: DatabaseConnection;
  defaultExpandedIds: Set<string>;
  onPreviewTable?: (table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onUseSql?: (sql: string) => void;
  schema?: DatabaseSchema;
  schemaLoading: boolean;
  status: DatabaseConnectionStatus;
  t: ReturnType<typeof useI18n>["t"];
  tableLookup: Map<string, DatabaseTable>;
}): TreeViewItem[] {
  if (status === "disconnected") {
    return [
      {
        disabled: true,
        id: `${connection.id}:disconnected`,
        label: "Connect to browse schema",
      },
    ];
  }

  if (schemaLoading) {
    return [
      {
        disabled: true,
        id: `${connection.id}:loading`,
        label: "Loading schema...",
      },
    ];
  }

  if (status === "failed") {
    return [
      {
        disabled: true,
        id: `${connection.id}:failed`,
        label: "Connection failed",
      },
    ];
  }

  if (!schema) {
    return [
      {
        disabled: true,
        id: `${connection.id}:schema-empty`,
        label: "Schema not loaded",
      },
    ];
  }

  if (!schema.tables.length) {
    return [
      {
        disabled: true,
        id: `${connection.id}:no-tables`,
        label: "No tables or views found",
      },
    ];
  }

  const databaseNodeId = `${connection.id}:database:${databaseLabel(connection)}`;
  defaultExpandedIds.add(databaseNodeId);

  if (connection.driver === "sqlite") {
    return [
      {
        children: schema.tables.map((table) =>
          tableItem({ connection, onPreviewTable, onUseSql, t, table, tableLookup }),
        ),
        icon: <Database size={13} />,
        id: databaseNodeId,
        label: databaseLabel(connection),
      },
    ];
  }

  const grouped = new Map<string, DatabaseTable[]>();
  for (const table of schema.tables) {
    const group = table.schema ?? "default";
    grouped.set(group, [...(grouped.get(group) ?? []), table]);
  }

  return [
    {
      children: [...grouped.entries()].map(([schemaName, tables]) => {
        const schemaNodeId = `${connection.id}:schema:${schemaName}`;
        defaultExpandedIds.add(schemaNodeId);
        return {
          actions: onRefreshSchema ? (
            <IconButton label={`Refresh ${schemaName} schema`} onClick={() => onRefreshSchema(connection)} size="compact">
              <RefreshCw size={12} />
            </IconButton>
          ) : undefined,
          children: tables.map((table) =>
            tableItem({ connection, onPreviewTable, onUseSql, t, table, tableLookup }),
          ),
          icon: <Columns3 size={13} />,
          id: schemaNodeId,
          label: schemaName,
          title: schemaName,
        };
      }),
      icon: <Database size={13} />,
      id: databaseNodeId,
      label: databaseLabel(connection),
      title: databaseLabel(connection),
    },
  ];
}

function tableItem({
  connection,
  onPreviewTable,
  onUseSql,
  t,
  table,
  tableLookup,
}: {
  connection: DatabaseConnection;
  onPreviewTable?: (table: DatabaseTable) => void;
  onUseSql?: (sql: string) => void;
  t: ReturnType<typeof useI18n>["t"];
  table: DatabaseTable;
  tableLookup: Map<string, DatabaseTable>;
}): TreeViewItem {
  const id = databaseTableTreeId(connection.id, table);
  tableLookup.set(id, table);
  return {
    actions: onPreviewTable ? (
      <IconButton label={`Open preview for ${table.name}`} onClick={() => onPreviewTable(table)} size="compact">
        <Play size={12} />
      </IconButton>
    ) : undefined,
    contextMenu: (
      <TableContextMenu
        connection={connection}
        onPreviewTable={onPreviewTable}
        onUseSql={onUseSql}
        t={t}
        table={table}
      />
    ),
    children: table.columns.map((column) => ({
      icon: <Columns3 size={12} />,
      id: `${id}:column:${column.name}`,
      label: column.name,
      meta: column.primaryKey ? <Badge tone="green">PK</Badge> : undefined,
      title: `${column.name} ${column.dataType}`,
    })),
    icon: <Table2 size={13} />,
    id,
    label: table.name,
    meta: <Badge tone="neutral">{table.kind}</Badge>,
    title: table.schema ? `${table.schema}.${table.name}` : table.name,
  };
}

function TableContextMenu({
  connection,
  onPreviewTable,
  onUseSql,
  t,
  table,
}: {
  connection: DatabaseConnection;
  onPreviewTable?: (table: DatabaseTable) => void;
  onUseSql?: (sql: string) => void;
  t: ReturnType<typeof useI18n>["t"];
  table: DatabaseTable;
}) {
  return (
    <>
      {onPreviewTable && (
        <ContextMenuItem onSelect={() => onPreviewTable(table)}>
          {t("database.tree.previewData")}
        </ContextMenuItem>
      )}
      {onUseSql && (
        <ContextMenuItem onSelect={() => onUseSql(generateSelectSql(connection.driver, table))}>
          {t("database.tree.generateSelect")}
        </ContextMenuItem>
      )}
      {onUseSql && (
        <ContextMenuItem onSelect={() => onUseSql(generateInsertSql(connection.driver, table))}>
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
  return table.schema ? `${quoteDbIdentifier(driver, table.schema)}.${name}` : name;
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

function ConnectionActions({
  connection,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onEditConnection,
  onNewQuery,
  onRefresh,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDeleteConnection?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onEditConnection?: (connection: DatabaseConnection) => void;
  onNewQuery?: () => void;
  onRefresh?: () => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();

  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton label={t("database.tree.actionsLabel", { name: connection.name })} size="compact">
          <MoreHorizontal size={13} />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem onSelect={() => onConnect?.(connection)}>{t("common.actions.connect")}</DropdownMenuItem>
        <DropdownMenuItem disabled={status === "disconnected"} onSelect={() => onDisconnect?.(connection)}>
          {t("common.actions.disconnect")}
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={onNewQuery}>{t("database.actions.newQuery")}</DropdownMenuItem>
        <DropdownMenuItem onSelect={onRefresh}>{t("database.actions.refreshConnections")}</DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onRefreshSchema?.(connection)}>{t("database.actions.refreshSchema")}</DropdownMenuItem>
        {onEditConnection && (
          <DropdownMenuItem onSelect={() => onEditConnection(connection)}>
            <Pencil size={13} />
            {t("database.tree.editConnection")}
          </DropdownMenuItem>
        )}
        <DropdownMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.name)}>
          <Copy size={13} />
          {t("database.tree.copyName")}
        </DropdownMenuItem>
        {onDeleteConnection && (
          <DropdownMenuItem
            className="text-[var(--u-color-danger)]"
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
  onNewQuery?: () => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  const { t } = useI18n();

  return (
    <>
      <ContextMenuItem onSelect={() => onConnect?.(connection)}>
        {t("common.actions.connect")}
      </ContextMenuItem>
      <ContextMenuItem
        disabled={status === "disconnected"}
        onSelect={() => onDisconnect?.(connection)}
      >
        {t("common.actions.disconnect")}
      </ContextMenuItem>
      <ContextMenuItem onSelect={onNewQuery}>{t("database.actions.newQuery")}</ContextMenuItem>
      <ContextMenuItem onSelect={() => onRefreshSchema?.(connection)}>
        {t("database.actions.refreshSchema")}
      </ContextMenuItem>
      {onEditConnection && (
        <ContextMenuItem onSelect={() => onEditConnection(connection)}>
          {t("database.tree.editConnection")}
        </ContextMenuItem>
      )}
      <ContextMenuItem onSelect={() => void navigator.clipboard?.writeText(connection.name)}>
        {t("database.tree.copyName")}
      </ContextMenuItem>
      {onDeleteConnection && (
        <ContextMenuItem onSelect={() => onDeleteConnection(connection)} tone="danger">
          {t("database.tree.deleteConnection")}
        </ContextMenuItem>
      )}
    </>
  );
}

function resolveConnectionStatus({
  hasSchema,
  selected,
  session,
}: {
  hasSchema: boolean;
  selected: boolean;
  session?: DatabaseConnectionSessionState;
}): DatabaseConnectionStatus {
  if (session?.status) {
    return session.status;
  }
  if (selected && hasSchema) {
    return "connected";
  }
  return "disconnected";
}

function databaseLabel(connection: DatabaseConnection) {
  if (connection.driver === "sqlite") {
    return connection.sqlitePath?.split(/[\\/]/).pop() || connection.name;
  }
  return connection.database || connection.name;
}

function connectionStateTitle(
  connection: DatabaseConnection,
  session?: DatabaseConnectionSessionState,
) {
  const message = session?.message ? ` - ${session.message}` : "";
  return `${connection.name} (${connection.driver})${message}`;
}

export function DatabaseSidebarToolbar({
  onNewQuery,
  onRefresh,
}: {
  onNewQuery?: () => void;
  onRefresh?: () => void;
}) {
  const { t } = useI18n();

  return (
    <div className="flex items-center gap-1">
      <IconButton label={t("database.actions.newQueryLabel")} onClick={onNewQuery}>
        <Play size={13} />
      </IconButton>
      <IconButton label={t("database.connection.refreshLabel")} onClick={onRefresh}>
        <RefreshCw size={13} />
      </IconButton>
    </div>
  );
}
