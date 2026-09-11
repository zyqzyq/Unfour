import { type Dispatch, type SetStateAction, useEffect, useRef } from "react";
import type {
  DatabaseConnection,
  DatabaseQueryResult,
  DatabaseTable,
  SavedSql,
} from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { useDatabaseTabs } from "./useDatabaseTabs";
import { useQueryHistory } from "./useQueryHistory";
import { useSavedSql } from "./useSavedSql";
import type {
  DatabaseQueryWorkspaceTab,
  DatabaseTableWorkspaceTab,
  SqlHistoryEntry,
} from "../model/types";
import { formatDatabaseError } from "../result-utils";

type DatabaseQueryWorkspaceActionsOptions = {
  activeQueryTab: DatabaseQueryWorkspaceTab | null;
  activeTableTab: DatabaseTableWorkspaceTab | null;
  browseTablePage: (
    connectionId: string,
    table: DatabaseTable,
    pageIndex: number,
    pageSize: number,
  ) => void;
  connections: DatabaseConnection[];
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  maxHistoryEntries: number;
  queryHistoryQuery: ReturnType<typeof useQueryHistory>;
  savedSqlQuery: ReturnType<typeof useSavedSql>;
  selectedConnectionId: string | null;
  setQueryHistory: Dispatch<SetStateAction<SqlHistoryEntry[]>>;
  setSelectedDatabaseConnection: (connectionId: string | null) => void;
  setSelectedTable: Dispatch<SetStateAction<DatabaseTable | null>>;
  t: ReturnType<typeof useI18n>["t"];
};

export function useDatabaseQueryWorkspaceActions({
  activeQueryTab,
  activeTableTab,
  browseTablePage,
  connections,
  databaseTabs,
  maxHistoryEntries,
  queryHistoryQuery,
  savedSqlQuery,
  selectedConnectionId,
  setQueryHistory,
  setSelectedDatabaseConnection,
  setSelectedTable,
  t,
}: DatabaseQueryWorkspaceActionsOptions) {
  const currentHistoryWorkspace = useRef(queryHistoryQuery.workspaceId);
  useEffect(() => { currentHistoryWorkspace.current = queryHistoryQuery.workspaceId; }, [queryHistoryQuery.workspaceId]);
  function startNewQuery(
    connectionId = selectedConnectionId ?? activeQueryTab?.connectionId ?? activeTableTab?.connectionId ?? null,
  ) {
    const tabId = databaseTabs.openQueryTab({ connectionId });
    if (connectionId) {
      setSelectedDatabaseConnection(connectionId);
    }
    setSelectedTable(null);
    return tabId;
  }

  function showQueryHistory() {
    const tabId = activeQueryTab?.id ?? databaseTabs.openQueryTab({ connectionId: selectedConnectionId });
    databaseTabs.updateQueryTab(tabId, { resultTab: "history" });
  }

  function recordSuccessfulHistory(
    result: DatabaseQueryResult,
    execution: { connectionId: string | null; sql: string } | null,
  ) {
    appendHistory({
      affectedRows: result.affectedRows,
      classification: result.safety.classification,
      connectionId: execution?.connectionId ?? null,
      connectionName: connectionNameForHistory(execution?.connectionId),
      durationMs: result.durationMs,
      rowCount: result.rows.length,
      sql: execution?.sql ?? "",
      status: "success",
    });
  }

  function recordFailedHistory(error: unknown, execution: { connectionId: string | null; sql: string } | null) {
    appendHistory({
      connectionId: execution?.connectionId ?? null,
      connectionName: connectionNameForHistory(execution?.connectionId),
      error: formatDatabaseError(error),
      sql: execution?.sql ?? "",
      status: "failed",
    });
  }

  function connectionNameForHistory(connectionId: string | null | undefined) {
    return connections.find((connection) => connection.id === connectionId)?.name ?? t("database.query.unknownConnection");
  }

  function appendHistory(entry: Omit<SqlHistoryEntry, "executedAt" | "id">) {
    const now = new Date().toISOString();
    const historyEntry: SqlHistoryEntry = {
      ...entry,
      executedAt: now,
      id: `${now}-${Math.random().toString(36).slice(2, 8)}`,
    };
    if (currentHistoryWorkspace.current === queryHistoryQuery.workspaceId) {
      setQueryHistory((current) => [historyEntry, ...current].slice(0, maxHistoryEntries));
    }
    queryHistoryQuery.record(historyEntry);
  }

  function clearQueryHistory() {
    setQueryHistory([]);
    queryHistoryQuery.clear();
  }

  function loadHistoryEntry(entry: SqlHistoryEntry) {
    const connectionId = connections.some((connection) => connection.id === entry.connectionId)
      ? entry.connectionId
      : null;
    databaseTabs.openQueryTab({
      connectionId,
      sql: entry.sql,
    });
    if (connectionId) {
      setSelectedDatabaseConnection(connectionId);
    }
    setSelectedTable(null);
  }

  // Open a saved SQL snippet from the sidebar tree into a fresh query tab.
  // Mirrors loadHistoryEntry: the connection id is honored only when the
  // owning connection still exists, otherwise the snippet opens without one.
  function openSavedSql(item: SavedSql) {
    const connectionId =
      item.connectionId && connections.some((connection) => connection.id === item.connectionId)
        ? item.connectionId
        : null;
    databaseTabs.openQueryTab({
      connectionId,
      sql: item.sql,
    });
    if (connectionId) {
      setSelectedDatabaseConnection(connectionId);
    }
    setSelectedTable(null);
  }

  function deleteSavedSql(item: SavedSql) {
    void savedSqlQuery.remove(item.id);
  }

  // Load generated SQL (e.g. from a table context-menu action) into a fresh editor tab.
  function loadSqlIntoEditor(connectionId: string, generatedSql: string, table?: DatabaseTable) {
    databaseTabs.openQueryTab({
      catalog: table?.catalog ?? null,
      connectionId,
      schema: table?.schema ?? null,
      sql: generatedSql,
    });
    setSelectedDatabaseConnection(connectionId);
    setSelectedTable(null);
  }

  function setActiveTabError(error: unknown) {
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, { error, resultTab: "results" });
      return;
    }
    if (activeTableTab) {
      databaseTabs.updateTableTab(activeTableTab.id, { error });
    }
  }

  function selectQueryConnection(connectionId: string | null) {
    setSelectedDatabaseConnection(connectionId);
    setSelectedTable(null);
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        catalog: null,
        connectionId,
        error: null,
        pendingConfirmation: false,
        schema: null,
      });
    }
  }

  function selectDatabaseTab(tabId: string) {
    const tab = databaseTabs.tabs.find((item) => item.id === tabId);
    databaseTabs.setActiveTabId(tabId);
    if (!tab) {
      return;
    }
    setSelectedDatabaseConnection(tab.connectionId);
    if (tab.kind === "table") {
      setSelectedTable(tab.table);
    } else {
      setSelectedTable(null);
    }
  }

  function designTable(connectionId: string, table: DatabaseTable) {
    const tabId = databaseTabs.openTableTab(connectionId, table, "structure");
    databaseTabs.updateTableTab(tabId, { segment: "structure" });
    setSelectedDatabaseConnection(connectionId);
    setSelectedTable(table);
  }

  function handleTablePageChange(pageIndex: number, pageSize: number) {
    if (!activeTableTab) {
      return;
    }
    browseTablePage(activeTableTab.connectionId, activeTableTab.table, pageIndex, pageSize);
  }

  function handleSelectResultTab(tab: DatabaseQueryWorkspaceTab["resultTab"]) {
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, { resultTab: tab });
    }
  }

  function handleSelectStructureTab(tab: DatabaseTableWorkspaceTab["structureTab"]) {
    if (activeTableTab) {
      databaseTabs.updateTableTab(activeTableTab.id, { structureTab: tab });
    }
  }

  function handleSelectTableSegment(segment: DatabaseTableWorkspaceTab["segment"]) {
    if (activeTableTab) {
      databaseTabs.updateTableTab(activeTableTab.id, { segment });
    }
  }

  function updateActiveSql(sql: string) {
    if (activeQueryTab) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: null,
        pendingConfirmation: false,
        sql,
      });
    }
  }


  return {
    clearQueryHistory,
    deleteSavedSql,
    designTable,
    handleSelectResultTab,
    handleSelectStructureTab,
    handleSelectTableSegment,
    handleTablePageChange,
    loadHistoryEntry,
    loadSqlIntoEditor,
    openSavedSql,
    recordFailedHistory,
    recordSuccessfulHistory,
    selectDatabaseTab,
    selectQueryConnection,
    setActiveTabError,
    showQueryHistory,
    startNewQuery,
    updateActiveSql,
  };
}
