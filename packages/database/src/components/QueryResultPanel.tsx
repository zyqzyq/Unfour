import { Clipboard, Download, Trash2 } from "lucide-react";
import { useState } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import { Button, EmptyState, ErrorState, LoadingState, StatusBadge, Tabs, Toolbar, ToolbarGroup, useI18n } from "@unfour/ui";
import type { DatabaseResultTab, SqlHistoryEntry } from "../model/types";
import { describeDatabaseError, serializeDatabaseResult, serializeDatabaseResultJson } from "../result-utils";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";
import { TableDataGrid } from "./TableDataGrid";

export function QueryResultPanel({
  activeTab,
  error,
  history,
  isPending,
  onClearHistory,
  onSelectHistory,
  onSelectTab,
  pendingConfirmation,
  result,
}: {
  activeTab: DatabaseResultTab;
  error: unknown;
  history: SqlHistoryEntry[];
  isPending: boolean;
  onClearHistory: () => void;
  onSelectHistory: (entry: SqlHistoryEntry) => void;
  onSelectTab: (tab: DatabaseResultTab) => void;
  pendingConfirmation: boolean;
  result: DatabaseQueryResult | null;
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
          { id: "results", title: "Results" },
          { id: "messages", title: "Messages" },
          { id: "logs", title: "Logs" },
          { id: "history", meta: <span className="text-[11px] text-[var(--u-color-text-soft)]">{history.length}</span>, title: t("database.history.tab") },
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
                {copyStatus === "copied" ? "Copied" : copyStatus === "failed" ? "Copy failed" : "Copy result"}
              </Button>
              <Button disabled={!result} onClick={exportCsv} size="sm" type="button" variant="outline">
                <Download size={13} />
                Export CSV
              </Button>
              <Button disabled={!result} onClick={exportJson} size="sm" type="button" variant="outline">
                <Download size={13} />
                Export JSON
              </Button>
            </>
          )}
        </ToolbarGroup>
      </Toolbar>
      {activeTab === "results" && renderResults({ error, isPending, pendingConfirmation, result })}
      {activeTab === "messages" && <Messages result={result} />}
      {activeTab === "logs" && <Logs error={error} isPending={isPending} result={result} />}
      {activeTab === "history" && <History entries={history} onSelect={onSelectHistory} />}
    </section>
  );
}

function renderResults({
  error,
  isPending,
  pendingConfirmation,
  result,
}: {
  error: unknown;
  isPending: boolean;
  pendingConfirmation: boolean;
  result: DatabaseQueryResult | null;
}) {
  if (error) {
    return (
      <ErrorState className="m-2 min-h-0 flex-1">
        <DatabaseErrorDetails confirmation={pendingConfirmation} error={error} />
      </ErrorState>
    );
  }

  if (isPending) {
    return <LoadingState className="m-2 min-h-0 flex-1">Running query...</LoadingState>;
  }

  if (!result) {
    return <EmptyState className="m-2 min-h-0 flex-1">Query results will appear here.</EmptyState>;
  }

  if (result.columns.length === 0) {
    return (
      <EmptyState className="m-2 min-h-0 flex-1">
        {result.affectedRows} rows affected in {result.durationMs}ms.
      </EmptyState>
    );
  }

  return <TableDataGrid result={result} />;
}

function Messages({ result }: { result: DatabaseQueryResult | null }) {
  return (
    <div className="min-h-0 flex-1 overflow-auto p-2 text-[12px] text-[var(--u-color-text-muted)]">
      {result ? (
        <div className="space-y-1">
          <div>{result.affectedRows} affected rows.</div>
          <div>Safety: {result.safety.classification}.</div>
          {result.safety.message ? <div>{result.safety.message}</div> : null}
          {result.columns.length > 0 ? <div>Read queries use the backend default limit unless the SQL includes an explicit limit.</div> : null}
        </div>
      ) : (
        "No messages."
      )}
    </div>
  );
}

function Logs({
  error,
  isPending,
  result,
}: {
  error: unknown;
  isPending: boolean;
  result: DatabaseQueryResult | null;
}) {
  const description = error ? describeDatabaseError(error) : null;
  return (
    <pre className="min-h-0 flex-1 overflow-auto p-2 font-mono text-[12px] text-[var(--u-color-text-muted)]">
      {isPending
        ? "Executing SQL..."
        : description
          ? description.technicalDetail ?? description.message
          : result
            ? `duration=${result.durationMs}ms rows=${result.rows.length} affected=${result.affectedRows} classification=${result.safety.classification}`
            : "No logs."}
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
