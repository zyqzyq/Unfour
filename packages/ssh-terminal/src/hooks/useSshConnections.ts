import { useQuery } from "@tanstack/react-query";
import { sshConnectionsQueryOptions } from "./sshWorkspaceQueries";

export function useSshConnections(
  workspaceId: string,
  options?: { active?: boolean },
) {
  const active = options?.active ?? true;
  return useQuery({
    ...sshConnectionsQueryOptions(workspaceId),
    enabled: Boolean(active && workspaceId),
  });
}
