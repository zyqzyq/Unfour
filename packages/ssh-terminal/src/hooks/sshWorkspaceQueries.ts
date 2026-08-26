import { queryOptions, type QueryClient } from "@tanstack/react-query";
import {
  listSshConnections,
  listSshSessions,
} from "@unfour/command-client";

export const SSH_WORKSPACE_STALE_TIME = 30_000;

export function sshConnectionsQueryKey(workspaceId: string) {
  return ["ssh-connections", workspaceId] as const;
}

export function sshSessionsQueryKey(workspaceId: string) {
  return ["ssh-sessions", workspaceId] as const;
}

export function sshConnectionsQueryOptions(workspaceId: string) {
  return queryOptions({
    queryKey: sshConnectionsQueryKey(workspaceId),
    queryFn: () => listSshConnections(workspaceId),
    staleTime: SSH_WORKSPACE_STALE_TIME,
  });
}

export function sshSessionsQueryOptions(workspaceId: string) {
  return queryOptions({
    queryKey: sshSessionsQueryKey(workspaceId),
    queryFn: () => listSshSessions(workspaceId),
    staleTime: SSH_WORKSPACE_STALE_TIME,
  });
}

export async function preloadSshWorkspace(
  queryClient: QueryClient,
  workspaceId: string,
) {
  if (!workspaceId) return;
  await Promise.all([
    queryClient.prefetchQuery(sshConnectionsQueryOptions(workspaceId)),
    queryClient.prefetchQuery(sshSessionsQueryOptions(workspaceId)),
  ]);
}
