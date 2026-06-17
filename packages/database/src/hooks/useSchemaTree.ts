import { useQuery } from "@tanstack/react-query";
import { getDatabaseSchema } from "@unfour/command-client";
import type { DatabaseConnection } from "@unfour/command-client";

export function useSchemaTree({
  connection,
  connectionId,
  enabled = true,
  workspaceId,
}: {
  connection: DatabaseConnection | null;
  connectionId: string | null;
  enabled?: boolean;
  workspaceId: string;
}) {
  return useQuery({
    enabled: Boolean(enabled && workspaceId && connectionId && connection),
    queryKey: ["database-schema", workspaceId, connectionId],
    queryFn: () => getDatabaseSchema(workspaceId, connectionId ?? ""),
  });
}
