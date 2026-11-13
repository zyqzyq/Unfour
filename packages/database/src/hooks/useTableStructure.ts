import { useQuery } from "@tanstack/react-query";
import { getDatabaseTableStructure, type DatabaseActionContext } from "@unfour/command-client";
import type { DatabaseTable } from "@unfour/command-client";

export function useTableStructure({
  connectionId,
  enabled = true,
  table,
  workspaceId,
  context,
}: {
  connectionId: string | null;
  enabled?: boolean;
  table: DatabaseTable | null;
  workspaceId: string;
  context?: DatabaseActionContext;
}) {
  return useQuery({
    enabled: Boolean(enabled && workspaceId && connectionId && table),
    queryKey: [
      "database-table-structure",
      workspaceId,
      connectionId,
      table?.catalog ?? null,
      table?.schema ?? null,
      table?.name ?? null,
      context,
    ],
    queryFn: () =>
      getDatabaseTableStructure({
        ...context,
        workspaceId,
        connectionId: connectionId ?? "",
        catalog: table?.catalog ?? null,
        schema: table?.schema ?? null,
        tableName: table?.name ?? "",
      }),
  });
}
