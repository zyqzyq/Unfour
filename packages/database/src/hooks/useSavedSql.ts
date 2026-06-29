import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteSavedSql,
  listSavedSql,
  saveSavedSql,
} from "@unfour/command-client";
import type { SavedSql, SavedSqlInput } from "@unfour/command-client";

export function savedSqlQueryKey(workspaceId: string) {
  return ["db-saved-sql", workspaceId] as const;
}

export function useSavedSql(workspaceId: string) {
  const queryClient = useQueryClient();
  const queryKey = savedSqlQueryKey(workspaceId);
  const query = useQuery({
    enabled: Boolean(workspaceId),
    queryKey,
    queryFn: () => listSavedSql(workspaceId),
  });

  const saveMutation = useMutation({
    mutationFn: (input: SavedSqlInput) => saveSavedSql(input),
    onSuccess: () => queryClient.invalidateQueries({ queryKey }),
  });

  const deleteMutation = useMutation({
    mutationFn: (id: string) => deleteSavedSql(workspaceId, id),
    onSuccess: (remaining: SavedSql[]) => {
      queryClient.setQueryData(queryKey, remaining);
    },
  });

  return {
    saved: query.data ?? [],
    error: query.error ?? saveMutation.error ?? deleteMutation.error,
    isLoading: query.isLoading,
    save: (input: SavedSqlInput) => saveMutation.mutateAsync(input),
    savePending: saveMutation.isPending,
    remove: (id: string) => deleteMutation.mutateAsync(id),
    removePending: deleteMutation.isPending,
  };
}
