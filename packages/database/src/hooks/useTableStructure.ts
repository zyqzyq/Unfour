import { useQuery } from "@tanstack/react-query";
import { getDatabaseTableStructure } from "@unfour/command-client";
import type { DatabaseTable } from "@unfour/command-client";

export function useTableStructure({
  connectionId,
  enabled = true,
  table,
  workspaceId,
}: {
  connectionId: string | null;
  enabled?: boolean;
  table: DatabaseTable | null;
  workspaceId: string;
}) {
  return useQuery({
    enabled: Boolean(enabled && workspaceId && connectionId && table),
    queryKey: [
      "database-table-structure",
      workspaceId,
      connectionId,
      table?.schema ?? null,
      table?.name ?? null,
    ],
    queryFn: () =>
      getDatabaseTableStructure({
        workspaceId,
        connectionId: connectionId ?? "",
        schema: table?.schema ?? null,
        tableName: table?.name ?? "",
      }),
  });
}
