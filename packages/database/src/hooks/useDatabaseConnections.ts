import { useQuery } from "@tanstack/react-query";
import {
  listDatabaseConnections,
  type DatabaseConnection,
} from "@unfour/command-client";

export function databaseConnectionsQueryKey(workspaceId: string) {
  return ["database-connections", workspaceId] as const;
}

export function replaceDatabaseConnectionInCache(
  current: DatabaseConnection[] | undefined,
  connection: DatabaseConnection,
): DatabaseConnection[] {
  const connections = current ?? [];
  const index = connections.findIndex((item) => item.id === connection.id);
  if (index === -1) {
    return [...connections, connection];
  }
  const next = connections.slice();
  next[index] = connection;
  return next;
}

export function resolveCachedDatabaseConnection(
  connection: DatabaseConnection,
  cached: DatabaseConnection[] | undefined,
): DatabaseConnection {
  return cached?.find((item) => item.id === connection.id) ?? connection;
}

export function useDatabaseConnections(
  workspaceId: string,
  options?: { active?: boolean },
) {
  const active = options?.active ?? true;
  return useQuery({
    enabled: Boolean(active && workspaceId),
    queryKey: databaseConnectionsQueryKey(workspaceId),
    queryFn: () => listDatabaseConnections(workspaceId),
  });
}
