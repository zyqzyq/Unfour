import { useMutation } from "@tanstack/react-query";
import { browseDatabaseTable } from "@unfour/command-client";
import type { DatabaseBrowseResult } from "@unfour/command-client";

export function useTableData({
  onBrowseStart,
  onSuccess,
  workspaceId,
}: {
  onBrowseStart: () => void;
  onSuccess: (result: DatabaseBrowseResult) => void;
  workspaceId: string;
}) {
  return useMutation({
    onMutate: onBrowseStart,
    mutationFn: ({
      catalog,
      connectionId,
      filter,
      orderBy,
      orderDescending,
      pageIndex,
      pageSize,
      schema,
      tableName,
    }: {
      catalog?: string | null;
      connectionId: string;
      filter?: string | null;
      orderBy?: string | null;
      orderDescending?: boolean;
      pageIndex: number;
      pageSize: number;
      schema?: string | null;
      tableName: string;
    }) =>
      browseDatabaseTable({
        workspaceId,
        connectionId,
        catalog,
        schema,
        tableName,
        limit: pageSize,
        offset: pageIndex * pageSize,
        orderBy: orderBy ?? null,
        orderDescending: orderDescending ?? false,
        filter: filter ?? null,
      }),
    onSuccess,
  });
}
