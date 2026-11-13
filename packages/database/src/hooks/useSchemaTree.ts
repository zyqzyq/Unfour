import { useQuery } from "@tanstack/react-query";
import { databaseActionContext, getDatabaseSchema } from "@unfour/command-client";
import type { DatabaseConnection } from "@unfour/command-client";

export function useSchemaTree({
  catalog = null,
  connection,
  connectionId,
  enabled = true,
  workspaceId,
}: {
  /** Catalog (database) to browse. Used by PostgreSQL to load a database other
   * than the connection default. Null loads the connection's default. */
  catalog?: string | null;
  connection: DatabaseConnection | null;
  connectionId: string | null;
  enabled?: boolean;
  workspaceId: string;
}) {
  const context = connection ? databaseActionContext(workspaceId, connection.id, connection) : undefined;
  return useQuery({
    enabled: Boolean(enabled && workspaceId && connectionId && connection),
    queryKey: ["database-schema", workspaceId, connectionId, catalog, context],
    queryFn: () => getDatabaseSchema(workspaceId, connectionId ?? "", catalog, context),
  });
}
