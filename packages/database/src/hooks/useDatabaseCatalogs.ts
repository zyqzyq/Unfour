import { useQuery } from "@tanstack/react-query";
import { databaseActionContext, listDatabaseCatalogs } from "@unfour/command-client";
import type { DatabaseConnection } from "@unfour/command-client";

// List the catalogs (databases) a connection can see. SQLite has none, so the
// query stays disabled for it; PostgreSQL and MySQL enumerate the server's
// databases so the query context can target beyond the connection default.
export function useDatabaseCatalogs({
  connection,
  connectionId,
  enabled = true,
  workspaceId,
}: {
  connection: DatabaseConnection | null;
  connectionId: string | null;
  enabled?: boolean;
  workspaceId: string;
}) {
  const supportsCatalogs = connection?.driver === "postgres" || connection?.driver === "mysql";

  const context = connection ? databaseActionContext(workspaceId, connection.id, connection) : undefined;
  return useQuery({
    enabled: Boolean(enabled && workspaceId && connectionId && supportsCatalogs),
    queryKey: ["database-catalogs", workspaceId, connectionId, context],
    queryFn: () => listDatabaseCatalogs(workspaceId, connectionId ?? "", context),
  });
}
