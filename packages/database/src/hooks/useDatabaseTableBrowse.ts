import { type Dispatch, type SetStateAction, useRef } from "react";
import type { DatabaseTable } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { databaseTableTabId, useDatabaseTabs } from "./useDatabaseTabs";
import { useTableData, type TableBrowseRequest } from "./useTableData";
import { useTableRowMutations } from "./useTableRowMutations";
import type {
  DatabaseConnectionSessionState,
  DatabaseTableWorkspaceTab,
  TableQueryState,
} from "../model/types";
import { emptyTableQuery } from "../model/types";
import { describeDatabaseError } from "../result-utils";

const DEFAULT_PREVIEW_PAGE_SIZE = 100;

export function useDatabaseTableBrowse({
  activeTableTab,
  databaseTabs,
  selectedConnectionId,
  selectedTable,
  setConnectionState,
  setSelectedTable,
  selectConnection,
  t,
  workspaceId,
}: {
  activeTableTab: DatabaseTableWorkspaceTab | null;
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  selectedConnectionId: string | null;
  selectedTable: DatabaseTable | null;
  setConnectionState: (
    connectionId: string,
    patch: Partial<DatabaseConnectionSessionState>,
  ) => void;
  setSelectedTable: Dispatch<SetStateAction<DatabaseTable | null>>;
  selectConnection: (connectionId: string | null) => void;
  t: ReturnType<typeof useI18n>["t"];
  workspaceId: string;
}) {
  const filterDebounceRef = useRef<number | null>(null);
  const browsingRef = useRef<TableBrowseRequest | null>(null);
  const cancelledBrowseRequestsRef = useRef(new WeakSet<TableBrowseRequest>());

  const browseMutation = useTableData({
    onBrowseStart: (request) => {
      databaseTabs.updateTableTab(request.tabId, {
        error: null,
        loading: true,
        segment: "data",
      });
    },
    onError: (error, request) => {
      if (cancelledBrowseRequestsRef.current.has(request)) {
        return;
      }
      const description = describeDatabaseError(error);
      databaseTabs.updateTableTab(request.tabId, { error, loading: false });
      if (["connection", "network", "permission"].includes(description.category)) {
        setConnectionState(request.connectionId, {
          message: description.message,
          status: "failed",
        });
      }
    },
    onSuccess: (browse, request) => {
      if (cancelledBrowseRequestsRef.current.has(request)) {
        return;
      }
      databaseTabs.updateTableTab(request.tabId, {
        error: null,
        loading: false,
        queryResult: browse.result,
        segment: "data",
        tableView: {
          pageIndex: Math.floor(browse.offset / Math.max(1, browse.limit)),
          pageSize: browse.limit,
          readOnly: browse.readOnly,
          tableName: browse.tableName,
          totalRows: browse.totalRows,
        },
      });
      setConnectionState(request.connectionId, {
        message: t("database.query.previewLoaded", {
          count: browse.result.rows.length,
        }),
        status: "connected",
      });
    },
    workspaceId,
  });

  function browseTablePage(
    connectionId: string,
    table: DatabaseTable,
    pageIndex: number,
    pageSize: number,
    query?: TableQueryState,
  ) {
    const existingTab = databaseTabs.tabs.find((tab) => tab.id === databaseTableTabId(connectionId, table));
    const effectiveQuery =
      query ?? (existingTab?.kind === "table" ? existingTab.tableQuery : { ...emptyTableQuery });
    const tabId = databaseTabs.openTableTab(connectionId, table, "data", true);
    if (connectionId !== selectedConnectionId) {
      selectConnection(connectionId);
    }
    setSelectedTable(table);
    databaseTabs.updateTableTab(tabId, {
      error: null,
      segment: "data",
      tableQuery: effectiveQuery,
    });
    browseMutation.reset();
    const request: TableBrowseRequest = {
      connectionId,
      catalog: table.catalog,
      pageIndex: Math.max(0, pageIndex),
      pageSize,
      schema: table.schema,
      tabId,
      tableName: table.name,
      orderBy: effectiveQuery.orderBy,
      orderDescending: effectiveQuery.orderDescending,
      filter: effectiveQuery.filter || null,
    };
    browsingRef.current = request;
    browseMutation.mutate(request);
  }

  function refreshTablePage() {
    if (activeTableTab?.tableView) {
      browseTablePage(
        activeTableTab.connectionId,
        activeTableTab.table,
        activeTableTab.tableView.pageIndex,
        activeTableTab.tableView.pageSize,
      );
    }
  }

  const { applyPendingTableChanges, rowMutation } = useTableRowMutations({
    activeTableTab,
    databaseTabs,
    refreshTablePage,
    workspaceId,
  });

  // Cycle a column through ascending -> descending -> unsorted, re-querying the
  // first page server-side each time.
  function applyTableSort(column: string) {
    if (!activeTableTab) {
      return;
    }
    const current = activeTableTab.tableQuery;
    let next: { orderBy: string | null; orderDescending: boolean; filter: string };
    if (current.orderBy !== column) {
      next = { ...current, orderBy: column, orderDescending: false };
    } else if (!current.orderDescending) {
      next = { ...current, orderDescending: true };
    } else {
      next = { ...current, orderBy: null, orderDescending: false };
    }
    browseTablePage(
      activeTableTab.connectionId,
      activeTableTab.table,
      0,
      activeTableTab.tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE,
      next,
    );
  }

  // Debounce the cross-column filter so typing does not fire a query per key.
  function applyTableFilter(text: string) {
    if (!activeTableTab) {
      return;
    }
    const next = { ...activeTableTab.tableQuery, filter: text };
    databaseTabs.updateTableTab(activeTableTab.id, { tableQuery: next });
    if (filterDebounceRef.current) {
      window.clearTimeout(filterDebounceRef.current);
    }
    const connectionId = activeTableTab.connectionId;
    const table = activeTableTab.table;
    const pageSize = activeTableTab.tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE;
    filterDebounceRef.current = window.setTimeout(() => {
      browseTablePage(connectionId, table, 0, pageSize, next);
    }, 350);
  }

  function previewSelectedTable() {
    if (!selectedConnectionId || !selectedTable) {
      return;
    }
    browseTablePage(
      selectedConnectionId,
      selectedTable,
      0,
      activeTableTab?.tableView?.pageSize ?? DEFAULT_PREVIEW_PAGE_SIZE,
    );
  }

  return {
    applyPendingTableChanges,
    applyTableFilter,
    applyTableSort,
    browseMutation,
    browseTablePage,
    browsingRef,
    cancelledBrowseRequestsRef,
    previewSelectedTable,
    rowMutation,
  };
}
