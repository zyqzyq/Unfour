import { Clipboard, Download, FileDown, FileJson, Info, Trash2 } from "lucide-react";
import { useState } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import {
  Button,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  EmptyState,
  ErrorState,
  IconButton,
  LoadingState,
  StatusBadge,
  Tabs,
  Toolbar,
  ToolbarGroup,
  useI18n,
} from "@unfour/ui";
import type { DatabaseResultTab, SqlHistoryEntry } from "../model/types";
import { describeDatabaseError, serializeDatabaseResult, serializeDatabaseResultJson } from "../result-utils";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";
import { TableDataGrid } from "./TableDataGrid";

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
}) {
  const { t } = useI18n();
  const [copyStatus, setCopyStatus] = useState<"idle" | "copied" | "failed">("idle");
  const [lastResult, setLastResult] = useState(result);
  if (result !== lastResult) {
    setLastResult(result);
    setCopyStatus("idle");
  }

  async function copyTsv() {
    if (!result) {
      return;
    }
    try {
      await navigator.clipboard.writeText(serializeDatabaseResult(result, "\t"));
      setCopyStatus("copied");
      window.setTimeout(() => setCopyStatus("idle"), 1600);
    } catch {
      setCopyStatus("failed");
    }
  }

  function downloadResult(content: string, mime: string, extension: string) {
    const blob = new Blob([content], { type: mime });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `unfour-query-results-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.${extension}`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }

  function exportCsv() {
    if (!result) {
      return;
    }
    downloadResult(serializeDatabaseResult(result, ","), "text/csv;charset=utf-8", "csv");
  }

  function exportJson() {
    if (!result) {
      return;
    }
    downloadResult(serializeDatabaseResultJson(result), "application/json;charset=utf-8", "json");
  }

  return (
    <section className="flex min-h-0 flex-1 flex-col bg-[var(--u-color-surface)]">
      <Tabs
        activeId={activeTab}
        className="h-[30px]"
        onSelect={(tabId) => onSelectTab(tabId as DatabaseResultTab)}
        tabs={[
          { id: "results", title: t("database.result.tabResults") },
          { id: "messages", title: t("database.result.tabMessages") },
          { id: "logs", title: t("database.result.tabLogs") },
          {
            id: "history",
            meta: <span className="text-[11px] text-[var(--u-color-text-soft)]">{history.length}</span>,
            title: t("database.history.tab"),
          },
        ]}
      />
      <Toolbar className="h-8">
        <ToolbarGroup>
          {error ? (
            <StatusBadge tone="danger">{t("database.result.statusFailed")}</StatusBadge>
          ) : result ? (
            <StatusBadge tone="success">{t("database.result.statusOk")}</StatusBadge>
          ) : null}
          <span className="text-[12px] text-[var(--u-color-text-muted)]">
            {result
              ? t("database.result.rowsInMs", { rows: result.rows.length, durationMs: result.durationMs })
              : error
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
            <>
              <Button disabled={!result} onClick={copyTsv} size="sm" type="button" variant="outline">
                <Clipboard size={13} />
                {copyStatus === "copied"
                  ? t("database.result.copied")
                  : copyStatus === "failed"
                    ? t("database.result.copyFailed")
                    : t("database.result.copy")}
              </Button>
              <DropdownMenu>
                <DropdownMenuTrigger asChild>
                  <IconButton disabled={!result} label={t("database.result.export")}>
                    <Download size={13} />
                  </IconButton>
                </DropdownMenuTrigger>
                <DropdownMenuContent>
                  <DropdownMenuItem disabled={!result} onSelect={exportCsv}>
                    <FileDown size={13} />
                    {t("database.result.exportCsv")}
                  </DropdownMenuItem>
                  <DropdownMenuItem disabled={!result} onSelect={exportJson}>
                    <FileJson size={13} />
                    {t("database.result.exportJson")}
                  </DropdownMenuItem>
                </DropdownMenuContent>
              </DropdownMenu>
            </>
          )}
        </ToolbarGroup>
      </Toolbar>
      {activeTab === "results" && results.length > 1 ? (
        <Tabs
          activeId={`result-${activeResultIndex}`}
          className="h-[28px] border-b border-[var(--u-color-border)]"
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
      {activeTab === "results" && renderResults({ error, isPending, pendingConfirmation, result, t })}
      {activeTab === "messages" && <Messages results={results} result={result} t={t} />}
      {activeTab === "logs" && <Logs error={error} isPending={isPending} results={results} result={result} t={t} />}
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

function renderResults({
  error,
  isPending,
  pendingConfirmation,
  result,
  t,
}: {
  error: unknown;
  isPending: boolean;
  pendingConfirmation: boolean;
  result: DatabaseQueryResult | null;
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
        <DatabaseErrorDetails confirmation={pendingConfirmation} error={error} />
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
        {t("database.result.affectedRows", { rows: result.affectedRows, durationMs: result.durationMs })}
      </EmptyState>
    );
  }

  return <TableDataGrid result={result} />;
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
            {item.columns.length > 0 ? <div>{t("database.result.limitHint")}</div> : null}
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
                    `#${index + 1} duration=${item.durationMs}ms rows=${item.rows.length} affected=${item.affectedRows} classification=${item.safety.classification}`,
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
          className="grid w-full cursor-pointer grid-cols-[86px_88px_minmax(0,1fr)_120px] items-center gap-2 border-b border-[var(--u-color-border)] px-2 py-1.5 text-left hover:bg-[var(--u-color-surface-hover)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-[var(--u-color-focus)]"
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
