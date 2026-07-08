import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  deleteSavedSql,
  listSavedSql,
  saveSavedSql,
} from "@unfour/command-client";
import type { SavedSql, SavedSqlInput } from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";

export function savedSqlQueryKey(workspaceId: string) {
  return ["db-saved-sql", workspaceId] as const;
}

export function useSavedSql(workspaceId: string) {
  const queryClient = useQueryClient();
  const handleError = useFeedbackErrorHandler();
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
    onError: (error) =>
      handleError(error, { key: "feedback.database.savedSqlDeleteFailed" }),
  });

  // Keep `saved` referentially stable: react-query preserves query.data across
  // renders when the data is unchanged, but `query.data ?? []` would allocate a
  // fresh empty array on every render while loading. Consumers that feed this
  // value into useMemo/useEffect dependencies (e.g. the sidebar tree builder,
  // which is pushed up to the shell via onShellSidebarChange) would otherwise
  // loop forever. Memoizing on query.data keeps the same reference until the
  // query result actually changes.
  const saved = useMemo(() => query.data ?? [], [query.data]);

  return {
    saved,
    // Delete failures surface via the feedback toast (see deleteMutation.onError),
    // so exclude deleteMutation.error here to avoid a duplicate inline ErrorState
    // and to keep it from leaking into the unrelated save dialog.
    error: query.error ?? saveMutation.error,
    isLoading: query.isLoading,
    save: (input: SavedSqlInput) => saveMutation.mutateAsync(input),
    savePending: saveMutation.isPending,
    remove: (id: string) => deleteMutation.mutateAsync(id),
    removePending: deleteMutation.isPending,
  };
}
