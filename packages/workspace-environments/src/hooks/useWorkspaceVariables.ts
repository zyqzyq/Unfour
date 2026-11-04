import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  listWorkspaceVariables,
  replaceWorkspaceVariables,
  type WorkspaceVariable,
  type WorkspaceVariableInput,
} from "@unfour/command-client";

export function useWorkspaceVariables(workspaceId: string) {
  const queryClient = useQueryClient();
  const queryKey = ["workspace-variables", workspaceId] as const;
  const query = useQuery({
    enabled: Boolean(workspaceId),
    queryKey,
    meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["workspace-variables"] },
    queryFn: () => listWorkspaceVariables(workspaceId),
  });
  const replaceMut = useMutation({
    mutationFn: ({ variables, expected }: { variables: WorkspaceVariableInput[]; expected: WorkspaceVariable[] }) =>
      replaceWorkspaceVariables(workspaceId, variables, expected),
    // Write the mutation result directly so the editor sync effect does not
    // briefly rehydrate from a stale list while invalidate is in flight.
    onSuccess: async (saved: WorkspaceVariable[]) => {
      await queryClient.cancelQueries({ queryKey });
      queryClient.setQueryData(queryKey, saved);
    },
    onError: () => queryClient.invalidateQueries({ queryKey }),
  });

  return {
    isLoading: query.isLoading,
    replaceMut,
    variables: query.data ?? [],
  };
}
