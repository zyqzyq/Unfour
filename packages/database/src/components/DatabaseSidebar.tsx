import { Database, Plus, RefreshCw } from "lucide-react";
import type {
  DatabaseConnection,
  DatabaseSchema,
  DatabaseTable,
  SavedSql,
} from "@unfour/command-client";
import { Badge, IconButton, useI18n } from "@unfour/ui";
import { DatabaseConnectionTree } from "./DatabaseConnectionTree";
import type { DatabaseConnectionSessionState } from "../model/types";

/**
 * Connection/schema tree mounted in the shell sidebar. The owning page pushes
 * this through `onShellSidebarChange`, the same pattern the API and SSH modules
 * use, so connection state stays in the feature package and the shell only
 * provides the mount surface.
 */
export function DatabaseSidebar({
  catalogNamesByConnection,
  connectionStates,
  connections,
  loadErrors,
  loadingKeys,
  onConnect,
  onDeleteConnection,
  onDeleteSavedSql,
  onDisconnect,
  onDuplicateConnection,
  onEditConnection,
  onNewConnection,
  onNewQuery,
  onOpenSavedSql,
  onPreviewTable,
  onRefresh,
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
  catalogNamesByConnection?: Record<string, string[]>;
  connectionStates?: Record<string, DatabaseConnectionSessionState>;
  connections: DatabaseConnection[];
  loadErrors?: Record<string, string>;
  loadingKeys?: string[];
  onConnect: (connection: DatabaseConnection) => void;
  onDeleteConnection: (connection: DatabaseConnection) => void;
  onDeleteSavedSql?: (item: SavedSql) => void;
  onDisconnect: (connection: DatabaseConnection) => void;
  onDuplicateConnection?: (connection: DatabaseConnection) => void;
  onEditConnection: (connection: DatabaseConnection) => void;
  onNewConnection: () => void;
  onNewQuery: (connection?: DatabaseConnection) => void;
  onOpenSavedSql?: (item: SavedSql) => void;
  onPreviewTable: (connectionId: string, table: DatabaseTable) => void;
  onRefresh: () => void;
  onRefreshSchema: (connection: DatabaseConnection) => void;
  onSelectConnection: (connection: DatabaseConnection) => void;
  onSelectTable: (connectionId: string, table: DatabaseTable) => void;
  onToggleCatalog: (connectionId: string, catalog: string) => void;
  onToggleConnection: (connection: DatabaseConnection) => void;
  onUseSql: (connectionId: string, sql: string, table?: DatabaseTable) => void;
  savedSqlByConnection?: Record<string, SavedSql[]>;
  schemaCache?: Record<string, DatabaseSchema>;
  selectedConnectionId: string | null;
  selectedTableId?: string | null;
}) {
  const { t } = useI18n();

  return (
    <section className="flex h-full min-h-0 flex-col space-y-1">
      <div className="flex h-7 shrink-0 items-center justify-between px-1">
        <span className="flex items-center gap-1.5 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          <Database size={13} />
          {t("database.sidebar.connections")}
          <Badge tone="neutral">{connections.length}</Badge>
        </span>
        <div className="flex items-center gap-1">
          <IconButton label={t("database.connection.newLabel")} onClick={onNewConnection} size="compact">
            <Plus size={13} />
          </IconButton>
          <IconButton label={t("database.connection.refreshLabel")} onClick={onRefresh} size="compact">
            <RefreshCw size={13} />
          </IconButton>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        <DatabaseConnectionTree
          catalogNamesByConnection={catalogNamesByConnection}
          connectionStates={connectionStates}
          connections={connections}
          loadErrors={loadErrors}
          loadingKeys={loadingKeys}
          onConnect={onConnect}
          onDeleteConnection={onDeleteConnection}
          onDeleteSavedSql={onDeleteSavedSql}
          onDisconnect={onDisconnect}
          onDuplicateConnection={onDuplicateConnection}
          onEditConnection={onEditConnection}
          onNewQuery={onNewQuery}
          onOpenSavedSql={onOpenSavedSql}
          onPreviewTable={onPreviewTable}
          onRefreshSchema={onRefreshSchema}
          onSelectConnection={onSelectConnection}
          onSelectTable={onSelectTable}
          onToggleCatalog={onToggleCatalog}
          onToggleConnection={onToggleConnection}
          onUseSql={onUseSql}
          savedSqlByConnection={savedSqlByConnection}
          schemaCache={schemaCache}
          selectedConnectionId={selectedConnectionId}
          selectedTableId={selectedTableId}
        />
      </div>
    </section>
  );
}
