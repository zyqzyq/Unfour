import { Database } from "lucide-react";
import { useState } from "react";
import type { DatabaseConnection, DatabaseSchema, DatabaseTable, SavedSql } from "@unfour/command-client";
import {
  ConfirmDialog,
  ConnectionStatus,
  EmptyState,
  StatusBadge,
  TreeView,
  useI18n,
  type TreeViewItem,
} from "@unfour/ui";
import { buildConnectionChildren, createDatabaseTreeIndex } from "./database-tree-content";
import type { DatabaseConnectionSessionState, DatabaseConnectionStatus } from "../model/types";
import {
  ConnectionContextMenu,
  ConnectionRowMenu,
} from "./database-tree-menus";
import { TableExportDialog } from "./TableExportDialog";


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
  onNewQuery?: (connection?: DatabaseConnection, catalog?: string) => void;
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
  const [pendingDelete, setPendingDelete] = useState<SavedSql | null>(null);
  const [exportTarget, setExportTarget] = useState<{ connection: DatabaseConnection; table: DatabaseTable } | null>(null);
  const requestDeleteSavedSql = onDeleteSavedSql
    ? (item: SavedSql) => setPendingDelete(item)
    : undefined;

  if (!connections.length) {
    return <EmptyState className="min-h-[72px]">{t("database.errors.noConnections")}</EmptyState>;
  }

  const index = createDatabaseTreeIndex();
  const { tableLookup, catalogLookup, savedSqlLookup, defaultExpandedIds } = index;
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
              index,
              catalogNames: catalogNamesByConnection?.[connection.id],
              connection,
              loadErrors,
              loadingKeys,
              onDeleteSavedSql: requestDeleteSavedSql,
              onDesignTable,
              onExportTable: (connection, table) => setExportTarget({ connection, table }),
              onOpenSavedSql,
              onNewQuery,
              onPreviewTable,
              onRefreshSchema,
              onUseSql,
              savedSql: savedSqlByConnection?.[connection.id],
              schemaCache,
              t,
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
    <>
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
    <ConfirmDialog
      confirmLabel={t("common.actions.delete")}
      description={
        pendingDelete ? t("database.tree.deleteSavedSqlBody", { name: pendingDelete.name }) : ""
      }
      onConfirm={() => {
        if (pendingDelete) {
          onDeleteSavedSql?.(pendingDelete);
        }
        setPendingDelete(null);
      }}
      onOpenChange={(open) => !open && setPendingDelete(null)}
      open={pendingDelete !== null}
      title={t("database.tree.deleteSavedSqlTitle")}
    />
    {exportTarget ? <TableExportDialog connection={exportTarget.connection} onOpenChange={(open) => !open && setExportTarget(null)} table={exportTarget.table} /> : null}
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
