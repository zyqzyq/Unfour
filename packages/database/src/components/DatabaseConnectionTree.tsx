import { Columns3, Database, MoreHorizontal, Play, RefreshCw, Table2 } from "lucide-react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable } from "@unfour/command-client";
import {
  Badge,
  ConnectionStatus,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  IconButton,
  TreeView,
  type TreeViewItem,
} from "@unfour/ui";
import { databaseTableTreeId } from "../model/database-tree";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";

export function DatabaseConnectionTree({
  connectionStates,
  connections,
  onConnect,
  onDisconnect,
  onNewQuery,
  onPreviewTable,
  onRefresh,
  onRefreshSchema,
  onSelectConnection,
  onSelectTable,
  schema,
  schemaLoading = false,
  selectedConnectionId,
  selectedTableId,
}: {
  connectionStates?: Record<string, DatabaseConnectionSessionState>;
  connections: DatabaseConnection[];
  onConnect?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onNewQuery?: () => void;
  onPreviewTable?: (table: DatabaseTable) => void;
  onRefresh?: () => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  onSelectConnection: (connection: DatabaseConnection) => void;
  onSelectTable?: (table: DatabaseTable) => void;
  schema?: DatabaseSchema;
  schemaLoading?: boolean;
  selectedConnectionId: string | null;
  selectedTableId?: string | null;
}) {
  if (!connections.length) {
    return <EmptyState className="min-h-[72px]">No database connections. Create one to browse schema or run SQL.</EmptyState>;
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
          onDisconnect={onDisconnect}
          onNewQuery={onNewQuery}
          onRefresh={onRefresh}
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
            schema,
            schemaLoading,
            status,
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
  schema,
  schemaLoading,
  status,
  tableLookup,
}: {
  connection: DatabaseConnection;
  defaultExpandedIds: Set<string>;
  onPreviewTable?: (table: DatabaseTable) => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  schema?: DatabaseSchema;
  schemaLoading: boolean;
  status: DatabaseConnectionStatus;
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
        children: schema.tables.map((table) => tableItem(connection, table, tableLookup, onPreviewTable)),
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
          children: tables.map((table) => tableItem(connection, table, tableLookup, onPreviewTable)),
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

function tableItem(
  connection: DatabaseConnection,
  table: DatabaseTable,
  tableLookup: Map<string, DatabaseTable>,
  onPreviewTable?: (table: DatabaseTable) => void,
): TreeViewItem {
  const id = databaseTableTreeId(connection.id, table);
  tableLookup.set(id, table);
  return {
    actions: onPreviewTable ? (
      <IconButton label={`Open preview for ${table.name}`} onClick={() => onPreviewTable(table)} size="compact">
        <Play size={12} />
      </IconButton>
    ) : undefined,
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

function ConnectionActions({
  connection,
  onConnect,
  onDisconnect,
  onNewQuery,
  onRefresh,
  onRefreshSchema,
  status,
}: {
  connection: DatabaseConnection;
  onConnect?: (connection: DatabaseConnection) => void;
  onDisconnect?: (connection: DatabaseConnection) => void;
  onNewQuery?: () => void;
  onRefresh?: () => void;
  onRefreshSchema?: (connection: DatabaseConnection) => void;
  status: DatabaseConnectionStatus;
}) {
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <IconButton label={`Database actions for ${connection.name}`} size="compact">
          <MoreHorizontal size={13} />
        </IconButton>
      </DropdownMenuTrigger>
      <DropdownMenuContent>
        <DropdownMenuItem onSelect={() => onConnect?.(connection)}>Connect</DropdownMenuItem>
        <DropdownMenuItem disabled={status === "disconnected"} onSelect={() => onDisconnect?.(connection)}>
          Disconnect
        </DropdownMenuItem>
        <DropdownMenuItem onSelect={onNewQuery}>New Query</DropdownMenuItem>
        <DropdownMenuItem onSelect={onRefresh}>Refresh Connections</DropdownMenuItem>
        <DropdownMenuItem onSelect={() => onRefreshSchema?.(connection)}>Refresh Schema</DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
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
  return (
    <div className="flex items-center gap-1">
      <IconButton label="New database query" onClick={onNewQuery}>
        <Play size={13} />
      </IconButton>
      <IconButton label="Refresh database connections" onClick={onRefresh}>
        <RefreshCw size={13} />
      </IconButton>
    </div>
  );
}
