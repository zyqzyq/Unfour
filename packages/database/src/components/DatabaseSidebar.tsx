import { Database, Plus, RefreshCw } from "lucide-react";
import type {
  DatabaseConnection,
  DatabaseSchema,
  DatabaseTable,
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
  connectionStates,
  connections,
  onConnect,
  onDeleteConnection,
  onDisconnect,
  onEditConnection,
  onNewConnection,
  onNewQuery,
  onPreviewTable,
  onRefresh,
  onRefreshSchema,
  onSelectConnection,
  onSelectTable,
  onUseSql,
  schema,
  schemaLoading,
  selectedConnectionId,
  selectedTableId,
}: {
  connectionStates?: Record<string, DatabaseConnectionSessionState>;
  connections: DatabaseConnection[];
  onConnect: (connection: DatabaseConnection) => void;
  onDeleteConnection: (connection: DatabaseConnection) => void;
  onDisconnect: (connection: DatabaseConnection) => void;
  onEditConnection: (connection: DatabaseConnection) => void;
  onNewConnection: () => void;
  onNewQuery: () => void;
  onPreviewTable: (table: DatabaseTable) => void;
  onRefresh: () => void;
  onRefreshSchema: (connection: DatabaseConnection) => void;
  onSelectConnection: (connection: DatabaseConnection) => void;
  onSelectTable: (table: DatabaseTable) => void;
  onUseSql: (sql: string) => void;
  schema?: DatabaseSchema;
  schemaLoading?: boolean;
  selectedConnectionId: string | null;
  selectedTableId?: string | null;
}) {
  const { t } = useI18n();

  return (
    <section className="flex min-h-0 flex-col space-y-1">
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
          connectionStates={connectionStates}
          connections={connections}
          onConnect={onConnect}
          onDeleteConnection={onDeleteConnection}
          onDisconnect={onDisconnect}
          onEditConnection={onEditConnection}
          onNewQuery={onNewQuery}
          onPreviewTable={onPreviewTable}
          onRefresh={onRefresh}
          onRefreshSchema={onRefreshSchema}
          onSelectConnection={onSelectConnection}
          onSelectTable={onSelectTable}
          onUseSql={onUseSql}
          schema={schema}
          schemaLoading={schemaLoading}
          selectedConnectionId={selectedConnectionId}
          selectedTableId={selectedTableId}
        />
      </div>
    </section>
  );
}
