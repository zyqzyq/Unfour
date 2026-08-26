import { useQuery } from "@tanstack/react-query";
import { sshSessionsQueryOptions } from "./sshWorkspaceQueries";

export function useTerminalSessions(
  workspaceId: string,
  options?: { active?: boolean },
) {
  const active = options?.active ?? true;
  return useQuery({
    ...sshSessionsQueryOptions(workspaceId),
    enabled: Boolean(active && workspaceId),
    // Keep Connections mounted for draft/session continuity, but stop fetching
    // while that surface is hidden so inactive modules stay idle.
    refetchInterval: active ? 2_000 : false,
  });
}
