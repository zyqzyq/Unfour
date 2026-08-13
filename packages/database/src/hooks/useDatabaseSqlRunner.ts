import { useRef, useState } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { useDatabaseTabs } from "./useDatabaseTabs";
import type { TableBrowseRequest } from "./useTableData";
import type {
  DatabaseConnectionSessionState,
  DatabaseQueryWorkspaceTab,
  RunSqlOptions,
} from "../model/types";
import { resolveExecutableStatements } from "../model/sql-statements";
import { executeSqlBatch, type SqlBatchState } from "../model/run-sql-batch";
import { describeDatabaseError } from "../result-utils";

export function useDatabaseSqlRunner({
  activeQueryTab,
  browseMutation,
  browsingRef,
  cancelledBrowseRequestsRef,
  databaseTabs,
  recordFailedHistory,
  recordSuccessfulHistory,
  setConnectionState,
  t,
  workspaceId,
}: {
  activeQueryTab: DatabaseQueryWorkspaceTab | null;
  browseMutation: { isPending: boolean; reset: () => void };
  browsingRef: { current: TableBrowseRequest | null };
  cancelledBrowseRequestsRef: { current: WeakSet<TableBrowseRequest> };
  databaseTabs: ReturnType<typeof useDatabaseTabs>;
  recordFailedHistory: (
    error: unknown,
    execution: { connectionId: string | null; sql: string } | null,
  ) => void;
  recordSuccessfulHistory: (
    result: DatabaseQueryResult,
    execution: { connectionId: string | null; sql: string } | null,
  ) => void;
  setConnectionState: (
    connectionId: string,
    patch: Partial<DatabaseConnectionSessionState>,
  ) => void;
  t: ReturnType<typeof useI18n>["t"];
  workspaceId: string;
}) {
  const cancelledRef = useRef(false);
  const executingRef = useRef<{ connectionId: string | null; sql: string; tabId: string } | null>(
    null,
  );
  const batchRef = useRef<SqlBatchState | null>(null);
  const [sqlRunning, setSqlRunning] = useState(false);

  function normalizeRunOptions(options?: string | RunSqlOptions): RunSqlOptions {
    if (typeof options === "string") {
      return { mode: "current", sql: options };
    }
    return options ?? { mode: "current" };
  }

  function applyQueryResults(tabId: string, collected: DatabaseQueryResult[], error: unknown = null) {
    const activeResultIndex = collected.length > 0 ? collected.length - 1 : 0;
    databaseTabs.updateQueryTab(tabId, {
      activeResultIndex,
      error,
      loading: false,
      result: collected[activeResultIndex] ?? null,
      results: collected,
      resultTab: "results",
    });
  }

  async function runSqlBatch(batch: SqlBatchState, confirmMutation: boolean) {
    cancelledRef.current = false;
    setSqlRunning(true);
    batchRef.current = batch;
    executingRef.current = {
      connectionId: batch.connectionId,
      sql: batch.statements[batch.nextIndex] ?? "",
      tabId: batch.tabId,
    };
    databaseTabs.updateQueryTab(batch.tabId, {
      error: null,
      loading: true,
      pendingConfirmation: false,
      resultTab: "results",
    });

    try {
      const outcome = await executeSqlBatch(batch, confirmMutation, {
        cancelled: () => cancelledRef.current,
        onConfirmationRequired: (paused, collected, error) => {
          batchRef.current = paused;
          executingRef.current = {
            connectionId: paused.connectionId,
            sql: paused.statements[paused.nextIndex] ?? "",
            tabId: paused.tabId,
          };
          databaseTabs.updateQueryTab(paused.tabId, {
            activeResultIndex: collected.length > 0 ? collected.length - 1 : 0,
            error,
            loading: false,
            pendingConfirmation: true,
            result: collected[collected.length - 1] ?? null,
            results: collected,
            resultTab: "results",
          });
        },
        onError: (current, collected, sql, error) => {
          executingRef.current = {
            connectionId: current.connectionId,
            sql,
            tabId: current.tabId,
          };
          applyQueryResults(current.tabId, collected, error);
          databaseTabs.updateQueryTab(current.tabId, { pendingConfirmation: false });
          recordFailedHistory(error, {
            connectionId: current.connectionId,
            sql,
          });
          const description = describeDatabaseError(error);
          if (["connection", "network", "permission"].includes(description.category)) {
            setConnectionState(current.connectionId, {
              message: description.message,
              status: "failed",
            });
          }
          batchRef.current = null;
        },
        onStatementSuccess: (current, collected, sql, result) => {
          executingRef.current = {
            connectionId: current.connectionId,
            sql,
            tabId: current.tabId,
          };
          batchRef.current = { ...current, collected, nextIndex: current.nextIndex + 1 };
          applyQueryResults(current.tabId, collected);
          recordSuccessfulHistory(result, {
            connectionId: current.connectionId,
            sql,
          });
          setConnectionState(current.connectionId, {
            message: t("database.query.completed", { durationMs: result.durationMs }),
            status: "connected",
          });
        },
        onSuccess: (current, collected) => {
          applyQueryResults(current.tabId, collected);
          databaseTabs.updateQueryTab(current.tabId, { pendingConfirmation: false });
          batchRef.current = null;
        },
        workspaceId,
      });

      if (outcome === "cancelled") {
        // stopQuery already wrote the cancelled error onto the tab.
        return;
      }
    } finally {
      setSqlRunning(false);
    }
  }

  function runSql(options?: string | RunSqlOptions) {
    browseMutation.reset();

    if (!activeQueryTab) {
      return;
    }

    const request = normalizeRunOptions(options);
    const pendingBatch =
      activeQueryTab.pendingConfirmation &&
      batchRef.current &&
      batchRef.current.tabId === activeQueryTab.id
        ? batchRef.current
        : null;

    if ((request.resume || activeQueryTab.pendingConfirmation) && pendingBatch) {
      void runSqlBatch(pendingBatch, true);
      return;
    }

    if (!activeQueryTab.connectionId) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: {
          code: "VALIDATION_ERROR",
          message: t("database.errors.selectBeforeRun"),
        },
        resultTab: "results",
      });
      return;
    }

    const statements = resolveExecutableStatements(activeQueryTab.sql, {
      mode: request.mode ?? "current",
      sql: request.sql,
      cursorOffset: request.cursorOffset,
    });

    if (!statements.length) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: {
          code: "VALIDATION_ERROR",
          message: t("database.errors.sqlEmpty"),
        },
        resultTab: "results",
      });
      return;
    }

    void runSqlBatch(
      {
        catalog: activeQueryTab.catalog,
        collected: [],
        connectionId: activeQueryTab.connectionId,
        nextIndex: 0,
        schema: activeQueryTab.schema,
        statements,
        tabId: activeQueryTab.id,
      },
      false,
    );
  }

  function clearSql() {
    if (!activeQueryTab) {
      return;
    }
    batchRef.current = null;
    databaseTabs.updateQueryTab(activeQueryTab.id, {
      activeResultIndex: 0,
      error: null,
      pendingConfirmation: false,
      result: null,
      results: [],
      sql: "",
    });
  }

  function selectQueryResult(index: number) {
    if (!activeQueryTab) {
      return;
    }
    const result = activeQueryTab.results[index];
    if (!result) {
      return;
    }
    databaseTabs.updateQueryTab(activeQueryTab.id, {
      activeResultIndex: index,
      result,
    });
  }

  // Stop a running query/preview. The in-flight statement keeps running
  // server-side until it finishes or hits its timeout, but late results are ignored.
  function stopQuery() {
    const stoppingExecution = sqlRunning ? executingRef.current : null;
    const stoppingBrowse = browseMutation.isPending ? browsingRef.current : null;
    const wasRunning = Boolean(sqlRunning || stoppingBrowse);
    if (!wasRunning) {
      return;
    }
    if (sqlRunning) {
      cancelledRef.current = true;
    }
    browseMutation.reset();
    setSqlRunning(false);
    const cancelledError = { code: "QUERY_CANCELLED", message: t("database.query.cancelled") };
    if (stoppingExecution) {
      databaseTabs.updateQueryTab(stoppingExecution.tabId, {
        error: cancelledError,
        loading: false,
        pendingConfirmation: false,
        resultTab: "results",
      });
    }
    if (stoppingBrowse) {
      cancelledBrowseRequestsRef.current.add(stoppingBrowse);
      databaseTabs.updateTableTab(stoppingBrowse.tabId, {
        error: cancelledError,
        loading: false,
      });
    }
    const connectionId = stoppingExecution?.connectionId ?? stoppingBrowse?.connectionId;
    if (connectionId) {
      setConnectionState(connectionId, {
        message: t("database.query.cancelled"),
        status: "connected",
      });
    }
  }

  return {
    clearSql,
    runSql,
    selectQueryResult,
    sqlRunning,
    stopQuery,
  };
}
