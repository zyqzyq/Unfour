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

export const MAX_HISTORY_ENTRIES = 25;

function historyEntries(rows: DbQueryHistoryEntry[]) {
  return rows.map(fromPersistedHistory);
}

export function useQueryHistoryEntries(workspaceId: string, limit: number, search: string, active: boolean) {
  const term = search.trim();
  return useQuery({
    enabled: Boolean(active && workspaceId),
    networkMode: "always",
    queryKey: [...dbQueryHistoryQueryKey(workspaceId), limit, term],
    queryFn: () => listDatabaseQueryHistory(workspaceId, limit, term || undefined),
    select: historyEntries,
  });
}

export function useQueryHistory(
  workspaceId: string,
  limit: number,
  options?: { active?: boolean },
) {
  const queryClient = useQueryClient();
  const query = useQueryHistoryEntries(workspaceId, limit, "", options?.active ?? true);

  const recordMutation = useMutation({
    networkMode: "always",
    mutationFn: (entry: DbQueryHistoryEntry) => recordDatabaseQueryHistory(entry),
    onSuccess: async (_result, entry) => {
      const filters = { queryKey: dbQueryHistoryQueryKey(entry.workspaceId) };
      // An initial list request may have read the database before this insert.
      await queryClient.cancelQueries(filters);
      await queryClient.invalidateQueries(filters);
    },
  });

  return {
    ...query,
    entries: query.data ?? [],
    record: (entry: SqlHistoryEntry) => recordMutation.mutate(toPersistedHistory(workspaceId, entry)),
  };
}

export function useClearQueryHistory() {
  const queryClient = useQueryClient();
  return useMutation({
    networkMode: "always",
    mutationFn: (workspaceId: string) => clearDatabaseQueryHistory(workspaceId),
    onSuccess: async (_result, workspaceId) => {
      const filters = { queryKey: dbQueryHistoryQueryKey(workspaceId) };
      await queryClient.cancelQueries(filters);
      queryClient.setQueriesData(filters, []);
      // A query can finish recording while the clear command is in flight.
      await queryClient.invalidateQueries(filters);
    },
  });
}

function fromPersistedHistory(entry: DbQueryHistoryEntry): SqlHistoryEntry {
  return {
    affectedRows: entry.affectedRows ?? undefined,
    classification: entry.classification ?? undefined,
    connectionId: entry.connectionId,
    catalog: entry.catalog ?? null,
    schema: entry.schema ?? null,
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
    catalog: entry.catalog ?? null,
    schema: entry.schema ?? null,
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
