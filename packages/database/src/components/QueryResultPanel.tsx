import { Info, Trash2 } from "lucide-react";
import { useMemo, useState } from "react";
import type { DatabaseQueryResult, DatabaseStatementResult } from "@unfour/command-client";
import {
  Button,
  EmptyState,
  ErrorState,
  LoadingState,
  StatusBadge,
  Tabs,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import type { DatabaseResultTab, SqlHistoryEntry } from "../model/types";
import { describeDatabaseError } from "../result-utils";
import { QueryResultOutputActions, type QueryResultOutputScope } from "./QueryResultOutputActions";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";
import { TableDataGrid } from "./TableDataGrid";
import { SqlBatchMessages } from "./SqlBatchMessages";
import { filterQueryResultRows, type QueryResultView } from "./query-result-view";
import type { SortState } from "./table-data-grid-helpers";

export function QueryResultPanel({
  activeResultIndex,
  activeTab,
  error,
  history,
  isPending,
  onClearHistory,
  onSelectHistory,
  onSelectResultSet,
  onSelectTab,
  pendingConfirmation,
  result,
  results,
  statements = [],
  executionNotice,
  confirmationSql,
}: {
  activeResultIndex: number;
  activeTab: DatabaseResultTab;
  error: unknown;
  history: SqlHistoryEntry[];
  isPending: boolean;
  onClearHistory: () => void;
  onSelectHistory: (entry: SqlHistoryEntry) => void;
  onSelectResultSet: (index: number) => void;
  onSelectTab: (tab: DatabaseResultTab) => void;
  pendingConfirmation: boolean;
  result: DatabaseQueryResult | null;
  results: DatabaseQueryResult[];
  statements?: DatabaseStatementResult[];
  executionNotice?: string | null;
  confirmationSql?: string | null;
}) {
  const { t } = useI18n();
  const statement = statements[activeResultIndex];
  const selectedError = pendingConfirmation ? null : error ?? statement?.error;
  const executionError = pendingConfirmation ? null : error;
  const skipped = statement?.status === "skipped";
  const displayResult = pendingConfirmation || isPending || selectedError || skipped ? null : result;
  const [scope, setScope] = useState<QueryResultOutputScope>("view");
  const [filter, setFilter] = useState("");
  const [sort, setSort] = useState<SortState | null>(null);
  const [page, setPage] = useState(0);
  const [lastResult, setLastResult] = useState(result);
  if (result !== lastResult) {
    setLastResult(result);
    setScope("view");
    setFilter("");
    setSort(null);
    setPage(0);
  }
  const viewRows = useMemo(() => filterQueryResultRows(displayResult?.rows ?? [], filter, sort), [displayResult, filter, sort]);
  const queryView: QueryResultView = {
    filter, sort, page, rows: viewRows,
    onFilter: setFilter, onSort: setSort, onPage: setPage,
  };
  const filtered = Boolean(filter.trim());

  return (
    <section className="flex min-h-0 min-w-0 flex-1 flex-col bg-[var(--u-color-canvas)]">
      <Tabs
        activeBackground="subtle"
        activeId={activeTab}
        className="h-9"
        onSelect={(tabId) => onSelectTab(tabId as DatabaseResultTab)}
        tabs={[
          { id: "results", title: t("database.result.tabResults") },
          { id: "messages", title: t("database.result.tabMessages") },
          { id: "logs", title: t("database.result.tabLogs") },
          {
            id: "history",
            meta: <span className="text-[length:var(--u-font-size-caption)] text-[var(--u-color-text-muted)]">{history.length}</span>,
            title: t("database.history.tab"),
          },
        ]}
      />
      <Toolbar className="h-auto min-h-10 flex-wrap gap-y-2 px-3 py-1.5">
        <ToolbarGroup className="flex-wrap gap-2">
          {pendingConfirmation ? (
            <StatusBadge tone="warning">{t("database.batch.confirmationRequired")}</StatusBadge>
          ) : selectedError ? (
            <StatusBadge tone="danger">{t("database.result.statusFailed")}</StatusBadge>
          ) : displayResult ? (
            <StatusBadge tone="success">{t("database.result.statusOk")}</StatusBadge>
          ) : null}
          {displayResult?.truncated ? (
            <StatusBadge tone="warning">{t("database.result.truncatedBadge")}</StatusBadge>
          ) : null}
          <span className="text-[12px] tabular-nums text-[var(--u-color-text-muted)]">
            {displayResult
              ? filtered
                ? t("database.result.filteredRowsInMs", { rows: displayResult.rows.length, matched: viewRows.length, durationMs: displayResult.durationMs })
                : t("database.result.rowsInMs", { rows: displayResult.rows.length, durationMs: displayResult.durationMs })
              : pendingConfirmation
                ? t("database.batch.confirmationPending")
                : selectedError
                  ? t("database.result.executionFailed")
                  : t("database.result.noExecution")}
          </span>
          {results.length > 1 ? (
            <span className="text-[12px] text-[var(--u-color-text-soft)]">
              {t("database.result.setCount", { count: results.length })}
            </span>
          ) : null}
        </ToolbarGroup>
        <ToolbarGroup>
          {activeTab === "history" ? (
            <Button disabled={!history.length} onClick={onClearHistory} size="sm" type="button" variant="outline">
              <Trash2 size={13} />
              {t("database.actions.clearHistory")}
            </Button>
          ) : (
            <QueryResultOutputActions result={displayResult} view={queryView} scope={scope} onScopeChange={setScope} />
          )}
        </ToolbarGroup>
      </Toolbar>
      {executionNotice ? <div role="status" className="px-2 py-1 text-[12px] text-[var(--u-color-text-muted)]">{executionNotice}</div> : null}
      {displayResult?.truncated ? (
        <div role="status" className="px-2 py-1 text-[12px] text-[var(--u-color-text-muted)]">
          {t("database.result.limitHint", { rows: displayResult.rows.length, limit: displayResult.rowLimit ?? displayResult.rows.length })}
        </div>
      ) : null}
      {pendingConfirmation && activeTab !== "history" ? (
        <ConfirmationPanel sql={confirmationSql ?? ""} t={t} />
      ) : null}
      {activeTab === "results" && !pendingConfirmation && statements.length ? (
        <>
          <Tabs
            activeId={`statement-${activeResultIndex}`}
            onSelect={(id) => onSelectResultSet(Number(id.replace("statement-", "")))}
            tabs={statements.map((entry, index) => ({
              id: `statement-${index}`,
              title: t("database.batch.statement", { index: entry.index, status: t(`database.batch.${entry.status}`) }),
            }))}
          />
          <pre className="max-h-24 shrink-0 overflow-auto border-b border-[var(--u-color-border)] p-2 text-[12px]" aria-label={t("database.batch.sql")}>{statement?.sql}</pre>
        </>
      ) : null}
      {activeTab === "results" && !pendingConfirmation && !statements.length && results.length > 1 ? (
        <Tabs
          activeId={`result-${activeResultIndex}`}
          className="h-9 border-b border-[var(--u-color-border)]"
          onSelect={(tabId) => {
            const index = Number(String(tabId).replace("result-", ""));
            if (Number.isFinite(index)) {
              onSelectResultSet(index);
            }
          }}
          tabs={results.map((item, index) => ({
            id: `result-${index}`,
            title: resultSetTitle(item, index, t),
          }))}
        />
      ) : null}
      {activeTab === "results" && !pendingConfirmation && (skipped
        ? <EmptyState>{t("database.batch.skippedDetail")}</EmptyState>
        : renderResults({ error: selectedError, isPending, result, queryView, t }))}
      {activeTab === "messages" && !pendingConfirmation && (executionError || isPending
        ? renderResults({ error: executionError, isPending, result: null, t })
        : statements.length ? <SqlBatchMessages statements={statements} /> : <Messages results={results} result={result} t={t} />)}
      {activeTab === "logs" && !pendingConfirmation && (statements.length ? <SqlBatchMessages statements={statements} /> : <Logs error={executionError} isPending={isPending} results={results} result={result} t={t} />)}
      {activeTab === "history" && <History entries={history} onSelect={onSelectHistory} />}
    </section>
  );
}

const QUERY_CANCELLED_CODE = "QUERY_CANCELLED";

function isCancelledError(error: unknown): error is { code: string; message?: string } {
  return (
    typeof error === "object" &&
    error !== null &&
    (error as { code?: unknown }).code === QUERY_CANCELLED_CODE
  );
}

function cancelledErrorMessage(error: unknown, fallback: string): string {
  const message = (error as { message?: unknown }).message;
  return typeof message === "string" && message.length > 0 ? message : fallback;
}

function resultSetTitle(
  result: DatabaseQueryResult,
  index: number,
  t: ReturnType<typeof useI18n>["t"],
) {
  if (result.columns.length > 0) {
    return t("database.result.setLabelRows", { index: index + 1, rows: result.rows.length });
  }
  return t("database.result.setLabelAffected", { index: index + 1, rows: result.affectedRows });
}

function ConfirmationPanel({
  sql,
  t,
}: {
  sql: string;
  t: ReturnType<typeof useI18n>["t"];
}) {
  return (
    <div
      className="m-2 flex min-h-0 flex-1 flex-col gap-2 rounded-[var(--u-radius-sm)] border border-[color:color-mix(in_srgb,var(--u-color-warning)_34%,var(--u-color-border))] bg-[var(--u-color-warning-soft)] p-3 text-left text-[12px] text-[var(--u-color-text)]"
      role="status"
    >
      <div className="font-semibold">{t("database.batch.confirmationRequired")}</div>
      <div className="text-[var(--u-color-text-muted)]">{t("database.batch.confirmationHint")}</div>
      {sql ? (
        <pre
          aria-label={t("database.batch.confirmationSql")}
          className="max-h-48 overflow-auto whitespace-pre-wrap break-words rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono text-[12px] text-[var(--u-color-text)]"
        >
          {sql}
        </pre>
      ) : null}
    </div>
  );
}

function renderResults({
  error,
  isPending,
  result,
  queryView,
  t,
}: {
  error: unknown;
  isPending: boolean;
  result: DatabaseQueryResult | null;
  queryView?: QueryResultView;
  t: ReturnType<typeof useI18n>["t"];
}) {
  if (error) {
    if (isCancelledError(error)) {
      // A cancelled query is a user-initiated action, not a failure – render it
      // as neutral info rather than a scary red error.
      return (
        <div
          className="m-2 flex min-h-0 flex-1 flex-col items-center justify-center gap-2 text-center text-[12px] text-[var(--u-color-text-soft)]"
          role="status"
        >
          <Info size={16} />
          <span>{cancelledErrorMessage(error, t("database.query.cancelled"))}</span>
        </div>
      );
    }
    return (
      <ErrorState className="m-2 min-h-0 flex-1">
        <DatabaseErrorDetails error={error} />
      </ErrorState>
    );
  }

  if (isPending) {
    return <LoadingState className="m-2 min-h-0 flex-1">{t("database.result.running")}</LoadingState>;
  }

  if (!result) {
    return <EmptyState className="m-2 min-h-0 flex-1">{t("database.result.empty")}</EmptyState>;
  }

  if (result.columns.length === 0) {
    return (
      <EmptyState className="m-2 min-h-0 flex-1">
        {result.safety.classification === "read"
          ? t("database.batch.emptyRows", { durationMs: result.durationMs })
          : t("database.result.affectedRows", { rows: result.affectedRows, durationMs: result.durationMs })}
      </EmptyState>
    );
  }

  return <TableDataGrid result={result} queryView={queryView} />;
}

function Messages({
  result,
  results,
  t,
}: {
  result: DatabaseQueryResult | null;
  results: DatabaseQueryResult[];
  t: ReturnType<typeof useI18n>["t"];
}) {
  if (!results.length && !result) {
    return (
      <div className="min-h-0 flex-1 overflow-auto p-2 text-[12px] text-[var(--u-color-text-muted)]">
        {t("database.result.noMessages")}
      </div>
    );
  }

  const items = results.length ? results : result ? [result] : [];
  return (
    <div className="min-h-0 flex-1 overflow-auto p-2 text-[12px] text-[var(--u-color-text-muted)]">
      <div className="space-y-3">
        {items.map((item, index) => (
          <div className="space-y-1" key={`message-${index}`}>
            {items.length > 1 ? (
              <div className="font-medium text-[var(--u-color-text)]">
                {t("database.result.setHeading", { index: index + 1 })}
              </div>
            ) : null}
            <div>{t("database.result.affectedRowsShort", { rows: item.affectedRows })}</div>
            <div>{t("database.result.safety", { classification: item.safety.classification })}</div>
            {item.safety.message ? <div>{item.safety.message}</div> : null}
            {item.truncated ? (
              <div>{t("database.result.limitHint", { rows: item.rows.length, limit: item.rowLimit ?? item.rows.length })}</div>
            ) : null}
          </div>
        ))}
      </div>
    </div>
  );
}

function Logs({
  error,
  isPending,
  result,
  results,
  t,
}: {
  error: unknown;
  isPending: boolean;
  result: DatabaseQueryResult | null;
  results: DatabaseQueryResult[];
  t: ReturnType<typeof useI18n>["t"];
}) {
  const description = error ? describeDatabaseError(error) : null;
  const items = results.length ? results : result ? [result] : [];
  return (
    <pre className="min-h-0 flex-1 overflow-auto p-2 font-mono text-[12px] text-[var(--u-color-text-muted)]">
      {isPending
        ? t("database.result.executingLog")
        : description
          ? description.technicalDetail ?? description.message
          : items.length
            ? items
                .map(
                  (item, index) =>
                    `#${index + 1} duration=${item.durationMs}ms rows=${item.rows.length} affected=${item.affectedRows} truncated=${item.truncated === true} rowLimit=${item.rowLimit ?? 0} classification=${item.safety.classification}`,
                )
                .join("\n")
            : t("database.result.noLogs")}
    </pre>
  );
}

function History({
  entries,
  onSelect,
}: {
  entries: SqlHistoryEntry[];
  onSelect: (entry: SqlHistoryEntry) => void;
}) {
  const { t } = useI18n();

  if (!entries.length) {
    return <EmptyState className="m-2 min-h-0 flex-1">{t("database.history.empty")}</EmptyState>;
  }

  return (
    <div className="min-h-0 flex-1 overflow-auto text-[12px]">
      {entries.map((entry) => (
        <button
          className="grid w-full cursor-pointer grid-cols-[72px_80px_minmax(0,1fr)_100px] items-center gap-3 border-b border-[var(--u-color-border)] px-3 py-3 text-left transition-colors hover:bg-[var(--u-color-surface-hover)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--u-color-focus)]"
          key={entry.id}
          onClick={() => onSelect(entry)}
          title={entry.sql}
          type="button"
        >
          <span className="text-[var(--u-color-text-soft)]">{formatHistoryTime(entry.executedAt)}</span>
          <StatusBadge tone={entry.status === "success" ? "success" : "danger"}>
            {entry.status === "success"
              ? t("database.history.statusSuccess")
              : t("database.history.statusFailed")}
          </StatusBadge>
          <span className="truncate font-mono text-[var(--u-color-text)]">{entry.sql}</span>
          <span className="truncate text-right text-[var(--u-color-text-soft)]">
            {entry.status === "success"
              ? t("database.history.rows", { count: entry.rowCount ?? entry.affectedRows ?? 0 })
              : entry.error ?? t("database.history.failedFallback")}
          </span>
        </button>
      ))}
    </div>
  );
}

function formatHistoryTime(value: string) {
  return new Date(value).toLocaleTimeString([], {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  });
}
