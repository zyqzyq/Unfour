import { useMemo } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  clearDatabaseQueryHistory,
  listDatabaseQueryHistory,
  recordDatabaseQueryHistory,
} from "@unfour/command-client";
import type { DbQueryHistoryEntry } from "@unfour/command-client";
import type { SqlHistoryEntry } from "../model/types";

export function dbQueryHistoryQueryKey(workspaceId: string) {
  return ["db-query-history", workspaceId] as const;
}

export function useQueryHistory(
  workspaceId: string,
  limit: number,
  options?: { active?: boolean },
) {
  const queryClient = useQueryClient();
  const queryKey = dbQueryHistoryQueryKey(workspaceId);
  const active = options?.active ?? true;
  const query = useQuery({
    enabled: Boolean(active && workspaceId),
    queryKey,
    queryFn: () => listDatabaseQueryHistory(workspaceId, limit),
  });
  const entries = useMemo(() => (query.data ?? []).map(fromPersistedHistory), [query.data]);

  const recordMutation = useMutation({
    mutationFn: (entry: SqlHistoryEntry) =>
      recordDatabaseQueryHistory(toPersistedHistory(workspaceId, entry)),
  });

  const clearMutation = useMutation({
    mutationFn: () => clearDatabaseQueryHistory(workspaceId),
    onMutate: () => {
      queryClient.setQueryData(queryKey, []);
    },
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey });
    },
  });

  return {
    ...query,
    clear: () => clearMutation.mutate(),
    entries,
    record: (entry: SqlHistoryEntry) => recordMutation.mutate(entry),
  };
}

function fromPersistedHistory(entry: DbQueryHistoryEntry): SqlHistoryEntry {
  return {
    affectedRows: entry.affectedRows ?? undefined,
    classification: entry.classification ?? undefined,
    connectionId: entry.connectionId,
    connectionName: entry.connectionName,
    durationMs: entry.durationMs ?? undefined,
    error: entry.error ?? undefined,
    executedAt: entry.executedAt,
    id: entry.id,
    rowCount: entry.rowCount ?? undefined,
    sql: entry.sql,
    status: entry.status,
  };
}

function toPersistedHistory(workspaceId: string, entry: SqlHistoryEntry): DbQueryHistoryEntry {
  return {
    affectedRows: entry.affectedRows ?? null,
    classification: entry.classification ?? null,
    connectionId: entry.connectionId,
    connectionName: entry.connectionName,
    durationMs: entry.durationMs ?? null,
    error: entry.error ?? null,
    executedAt: entry.executedAt,
    id: entry.id,
    rowCount: entry.rowCount ?? null,
    sql: entry.sql,
    status: entry.status,
    workspaceId,
  };
}
