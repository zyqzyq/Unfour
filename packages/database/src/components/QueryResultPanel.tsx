import { Clipboard, Download } from "lucide-react";
import { useState } from "react";
import type { DatabaseQueryResult } from "@unfour/command-client";
import { Button, EmptyState, ErrorState, LoadingState, StatusBadge, Tabs, Toolbar, ToolbarGroup } from "@unfour/ui";
import type { DatabaseResultTab, SqlHistoryEntry } from "../model/types";
import { describeDatabaseError, serializeDatabaseResult } from "../result-utils";
import { DatabaseErrorDetails } from "./DatabaseErrorDetails";
import { TableDataGrid } from "./TableDataGrid";

export function QueryResultPanel({
  activeTab,
  error,
  history,
  isPending,
  onSelectHistory,
  onSelectTab,
  pendingConfirmation,
  result,
}: {
  activeTab: DatabaseResultTab;
  error: unknown;
  history: SqlHistoryEntry[];
  isPending: boolean;
  onSelectHistory: (entry: SqlHistoryEntry) => void;
  onSelectTab: (tab: DatabaseResultTab) => void;
  pendingConfirmation: boolean;
  result: DatabaseQueryResult | null;
}) {
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

  function exportCsv() {
    if (!result) {
      return;
    }
    const blob = new Blob([serializeDatabaseResult(result, ",")], { type: "text/csv;charset=utf-8" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `unfour-query-results-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, "-")}.csv`;
    document.body.appendChild(link);
    link.click();
    link.remove();
    URL.revokeObjectURL(url);
  }

  return (
    <section className="flex min-h-[190px] flex-[0.45] flex-col border-t border-[var(--u-color-border)] bg-[var(--u-color-surface)]">
      <Tabs
        activeId={activeTab}
        className="h-[30px]"
        onSelect={(tabId) => onSelectTab(tabId as DatabaseResultTab)}
        tabs={[
          { id: "results", title: "Results" },
          { id: "messages", title: "Messages" },
          { id: "logs", title: "Logs" },
          { id: "history", meta: <span className="text-[11px] text-[var(--u-color-text-soft)]">{history.length}</span>, title: "History" },
        ]}
      />
      <Toolbar className="h-8">
        <ToolbarGroup>
          <span className="text-[12px] text-[var(--u-color-text-muted)]">
            {result ? `${result.rows.length} rows in ${result.durationMs}ms` : error ? "Execution failed" : "No execution yet"}
          </span>
        </ToolbarGroup>
        <ToolbarGroup>
          <Button disabled={!result} onClick={copyTsv} size="sm" type="button" variant="outline">
            <Clipboard size={13} />
            {copyStatus === "copied" ? "Copied" : copyStatus === "failed" ? "Copy failed" : "Copy result"}
          </Button>
          <Button disabled={!result} onClick={exportCsv} size="sm" type="button" variant="outline">
            <Download size={13} />
            Export CSV
          </Button>
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
  if (!entries.length) {
    return <EmptyState className="m-2 min-h-0 flex-1">Executed SQL history will appear here for this session.</EmptyState>;
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
          <StatusBadge tone={entry.status === "success" ? "success" : "danger"}>{entry.status}</StatusBadge>
          <span className="truncate font-mono text-[var(--u-color-text)]">{entry.sql}</span>
          <span className="truncate text-right text-[var(--u-color-text-soft)]">
            {entry.status === "success"
              ? `${entry.rowCount ?? entry.affectedRows ?? 0} rows`
              : entry.error ?? "failed"}
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
