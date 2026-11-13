import { useQuery, type QueryClient } from "@tanstack/react-query";
import { listRedisConnections, type RedisConnection } from "@unfour/command-client";

export function redisConnectionsQueryKey(workspaceId: string) {
  return ["redis-connections", workspaceId] as const;
}

export async function updateRedisConnectionsAfterMutation(
  client: QueryClient,
  record: RedisConnection,
  operation: "save" | "delete",
) {
  const queryKey = redisConnectionsQueryKey(record.workspaceId);
  await client.cancelQueries({ queryKey, exact: true });
  client.setQueryData<RedisConnection[]>(queryKey, (current = []) => {
    const previous = current.find((item) => item.id === record.id);
    if (previous && previous.revision > record.revision) return current;
    if (operation === "delete") return current.filter((item) => item.id !== record.id);
    if (!previous) return [...current, record];
    return current.map((item) => item.id === record.id ? record : item);
  });
  await client.invalidateQueries({ queryKey, exact: true });
}

export function useRedisConnections(workspaceId: string, active: boolean) {
  return useQuery({
    enabled: Boolean(workspaceId && active),
    queryKey: redisConnectionsQueryKey(workspaceId),
    queryFn: () => listRedisConnections(workspaceId),
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["database-connections"] },
  });
}
