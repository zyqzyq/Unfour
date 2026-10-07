import { useRef } from "react";
import { useMutation } from "@tanstack/react-query";
import { browseDatabaseTable } from "@unfour/command-client";
import type { DatabaseBrowseResult } from "@unfour/command-client";

export type TableBrowseRequest = {
  catalog?: string | null;
  connectionId: string;
  filter?: string | null;
  orderBy?: string | null;
  orderDescending?: boolean;
  pageIndex: number;
  pageSize: number;
  schema?: string | null;
  tabId: string;
  tableName: string;
};

export function useTableData({
  onBrowseStart,
  onError,
  onSuccess,
  workspaceId,
}: {
  onBrowseStart: (request: TableBrowseRequest) => void;
  onError?: (error: unknown, request: TableBrowseRequest) => void;
  onSuccess: (result: DatabaseBrowseResult, request: TableBrowseRequest) => void;
  workspaceId: string;
}) {
  const latestRequestByTab = useRef(new Map<string, TableBrowseRequest>());

  return useMutation({
    onMutate: (request: TableBrowseRequest) => {
      latestRequestByTab.current.set(request.tabId, request);
      onBrowseStart(request);
    },
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
    }: TableBrowseRequest) =>
      browseDatabaseTable({
        includeTotal: false,
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
    onError: (error, request) => {
      if (latestRequestByTab.current.get(request.tabId) === request) {
        latestRequestByTab.current.delete(request.tabId);
        onError?.(error, request);
      }
    },
    onSuccess: (result, request) => {
      if (latestRequestByTab.current.get(request.tabId) === request) {
        latestRequestByTab.current.delete(request.tabId);
        onSuccess(result, request);
      }
    },
  });
}
