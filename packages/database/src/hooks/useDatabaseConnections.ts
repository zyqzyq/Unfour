import { useQuery } from "@tanstack/react-query";
import { listDatabaseConnections } from "@unfour/command-client";

export function useDatabaseConnections(
  workspaceId: string,
  options?: { active?: boolean },
) {
  const active = options?.active ?? true;
  return useQuery({
    enabled: Boolean(active && workspaceId),
    queryKey: ["database-connections", workspaceId],
    queryFn: () => listDatabaseConnections(workspaceId),
  });
}
