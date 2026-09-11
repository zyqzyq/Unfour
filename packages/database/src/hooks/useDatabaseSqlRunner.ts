import { useRef, useState } from "react";
import { stopDatabaseScript, type DatabaseQueryResult } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { useDatabaseTabs } from "./useDatabaseTabs";
import type { TableBrowseRequest } from "./useTableData";
import type {
  DatabaseConnectionSessionState,
  DatabaseQueryWorkspaceTab,
  RunSqlOptions,
} from "../model/types";

import { canConfirmBatch, createSqlBatch, executeSqlBatch, sqlAwaitingConfirmation, type SqlBatchState } from "../model/run-sql-batch";
import { describeDatabaseError, isConfirmationRequired } from "../result-utils";

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
  const activeRunRef = useRef<SqlBatchState | null>(null);
  const confirmationRef = useRef<SqlBatchState | null>(null);
  const stopRequestedRef = useRef(false);
  const [sqlRunning, setSqlRunning] = useState(false);

  async function runSqlBatch(batch: SqlBatchState, confirmed: boolean) {
    if (activeRunRef.current) return;
    activeRunRef.current = batch;
    stopRequestedRef.current = false;
    confirmationRef.current = null;
    setSqlRunning(true);
    databaseTabs.updateQueryTab(batch.tabId, {
      error: null, loading: true, pendingConfirmation: false,
      result: null, results: [], statements: [], activeResultIndex: 0, resultTab: "results",
      executionNotice: null,
      executionRunId: batch.input.runId, confirmationSql: null,
    });
    try {
      const output = await executeSqlBatch(batch, confirmed);
      const results = output.statements.flatMap((entry) => entry.result ? [entry.result] : []);
      const failedIndex = output.statements.findIndex((entry) => entry.status === "failed");
      const index = failedIndex >= 0 ? failedIndex : Math.max(0, results.length - 1);
      const selected = output.statements[index];
      databaseTabs.updateQueryTab(batch.tabId, {
        activeResultIndex: index, error: null, loading: false, pendingConfirmation: false,
        result: selected?.result ?? null, results, statements: output.statements, resultTab: "results",
        executionNotice: [t(output.stopped ? "database.batch.stopped" : "database.batch.sessionEnded"), ...(output.warnings ?? []).map((key) => t(key))].join(" "),
        executionRunId: null,
      });
      for (const entry of output.statements) {
        const execution = { connectionId: batch.input.connectionId, sql: entry.sql };
        if (entry.result) recordSuccessfulHistory(entry.result, execution);
        if (entry.status === "failed") recordFailedHistory(entry.error, execution);
      }
      const failed = output.statements.find((entry) => entry.status === "failed");
      setConnectionState(batch.input.connectionId, { status: "connected", message: null });
      if (failed) updateConnectionError(batch.input.connectionId, failed.error);
    } catch (error) {
      const cancelledPreflight = stopRequestedRef.current && isConfirmationRequired(error);
      const confirmation = !cancelledPreflight && isConfirmationRequired(error);
      confirmationRef.current = confirmation ? batch : null;
      databaseTabs.updateQueryTab(batch.tabId, {
        error: cancelledPreflight ? null : error, loading: false, pendingConfirmation: confirmation, resultTab: "results",
        executionNotice: cancelledPreflight ? t("database.batch.stopped") : null,
        executionRunId: null, confirmationSql: confirmation ? sqlAwaitingConfirmation(batch, error) : null,
      });
      // Preflight/transport failures are not statement executions.
      if (!confirmation) updateConnectionError(batch.input.connectionId, error);
    } finally {
      if (activeRunRef.current === batch) {
        activeRunRef.current = null;
        setSqlRunning(false);
      }
    }
  }

  function updateConnectionError(connectionId: string, error: unknown) {
    const description = describeDatabaseError(error);
    if (["connection", "network"].includes(description.category)) {
      setConnectionState(connectionId, { message: description.message, status: "failed" });
    }
  }

  function runSql(options?: string | RunSqlOptions) {
    if (!activeQueryTab || activeRunRef.current || activeQueryTab.loading) return;
    browseMutation.reset();
    const request = typeof options === "string" ? { mode: "current" as const, sql: options } : options ?? {};
    if (request.cancelConfirmation) {
      confirmationRef.current = null;
      databaseTabs.updateQueryTab(activeQueryTab.id, { error: null, pendingConfirmation: false, confirmationSql: null });
      return;
    }
    const pending = confirmationRef.current;
    if (request.resume && pending && canConfirmBatch(pending, activeQueryTab, workspaceId)) {
      void runSqlBatch(pending, true);
      return;
    }
    confirmationRef.current = null;
    if (!activeQueryTab.connectionId || !(request.sql ?? activeQueryTab.sql).trim()) {
      databaseTabs.updateQueryTab(activeQueryTab.id, {
        error: { code: "VALIDATION_ERROR", message: t(activeQueryTab.connectionId ? "database.errors.sqlEmpty" : "database.errors.selectBeforeRun") },
        pendingConfirmation: false, resultTab: "results",
      });
      return;
    }
    void runSqlBatch(createSqlBatch(activeQueryTab, request, workspaceId), false);
  }

  function clearSql() {
    if (!activeQueryTab || activeRunRef.current || activeQueryTab.loading) return;
    confirmationRef.current = null;
    databaseTabs.updateQueryTab(activeQueryTab.id, {
      activeResultIndex: 0, error: null, pendingConfirmation: false,
      result: null, results: [], statements: [], executionNotice: null, confirmationSql: null, sql: "",
    });
  }

  function selectQueryResult(index: number) {
    if (!activeQueryTab) return;
    const statement = activeQueryTab.statements?.[index];
    const result = statement ? statement.result : activeQueryTab.results[index];
    if (!statement && !result) return;
    databaseTabs.updateQueryTab(activeQueryTab.id, { activeResultIndex: index, result: result ?? null });
  }

  function stopQuery() {
    const batch = activeRunRef.current;
    if (!batch && activeQueryTab?.executionRunId) {
      const tabId = activeQueryTab.id;
      databaseTabs.updateQueryTab(tabId, { executionNotice: t("database.batch.stopping") });
      void stopDatabaseScript(workspaceId, activeQueryTab.executionRunId).catch(() => {
        databaseTabs.updateQueryTab(tabId, { executionNotice: t("database.batch.stopRetry") });
      });
      return;
    }
    if (batch) {
      stopRequestedRef.current = true;
      databaseTabs.updateQueryTab(batch.tabId, { executionNotice: t("database.batch.stopping") });
      void stopDatabaseScript(batch.input.workspaceId, batch.input.runId).then((accepted) => {
        if (activeRunRef.current !== batch) return;
        databaseTabs.updateQueryTab(batch.tabId, {
          executionNotice: t(accepted ? "database.batch.stopping" : "database.batch.stopRetry"),
        });
      }).catch(() => {
        if (activeRunRef.current === batch) databaseTabs.updateQueryTab(batch.tabId, { executionNotice: t("database.batch.stopRetry") });
      });
      // Run stays disabled until the real outcome arrives.
      return;
    }
    const browse = browseMutation.isPending ? browsingRef.current : null;
    if (!browse) return;
    cancelledBrowseRequestsRef.current.add(browse);
    browseMutation.reset();
    databaseTabs.updateTableTab(browse.tabId, {
      error: { code: "QUERY_CANCELLED", message: t("database.query.cancelled") }, loading: false,
    });
  }

  return { clearSql, runSql, selectQueryResult, sqlRunning, stopQuery };
}
