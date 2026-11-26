import { Trash2 } from "lucide-react";
import { useEffect, useState } from "react";
import { Button, ConfirmDialog, EmptyState, ErrorState, Input, LoadingState, StatusBadge, Toolbar, useI18n } from "@unfour/ui";
import { MAX_HISTORY_ENTRIES, useClearQueryHistory, useQueryHistoryEntries } from "../hooks/useQueryHistory";
import type { SqlHistoryEntry } from "../model/types";
import { formatDatabaseError } from "../result-utils";

export function QueryHistoryPanel({ active, hasHistory, onSearch, onSelect, search, workspaceId, workspaceName }: {
  active: boolean;
  hasHistory: boolean;
  onSearch: (value: string) => void;
  onSelect: (entry: SqlHistoryEntry) => void;
  search: string;
  workspaceId: string;
  workspaceName: string;
}) {
  const { t, locale } = useI18n();
  const term = search.trim();
  const [debounced, setDebounced] = useState(term);
  const [confirmOpen, setConfirmOpen] = useState(false);
  useEffect(() => {
    if (term === debounced) return;
    const timer = window.setTimeout(() => setDebounced(term), 180);
    return () => window.clearTimeout(timer);
  }, [debounced, term]);
  const waiting = Boolean(term && term !== debounced);
  const history = useQueryHistoryEntries(workspaceId, MAX_HISTORY_ENTRIES, term ? debounced : "", active && !waiting);
  const clear = useClearQueryHistory();
  const entries = history.data ?? [];

  return (
    <>
      <Toolbar className="h-auto min-h-10 flex-wrap gap-2 px-3 py-1.5">
        <Input
          aria-label={t("database.history.search")}
          className="h-7 w-60 max-w-full"
          onChange={(event) => onSearch(event.target.value)}
          placeholder={t("database.history.search")}
          type="search"
          value={search}
        />
        <Button disabled={!hasHistory && entries.length === 0} onClick={() => { clear.reset(); setConfirmOpen(true); }} size="sm" variant="outline">
          <Trash2 size={13} />
          {t("database.actions.clearHistory")}
        </Button>
      </Toolbar>
      {waiting || history.isPending ? (
        <LoadingState>{t("common.state.loading")}</LoadingState>
      ) : history.isError ? (
        <ErrorState>{formatDatabaseError(history.error, t)}{" "}
          <Button size="sm" variant="outline" onClick={() => void history.refetch()}>{t("common.actions.retry")}</Button>
        </ErrorState>
      ) : entries.length === 0 ? (
        <EmptyState className="m-2 min-h-0 flex-1">{t(term ? "database.history.noMatches" : "database.history.empty")}</EmptyState>
      ) : (
        <div className="min-h-0 flex-1 overflow-auto text-[12px]">
          {entries.map((entry) => (
            <button
              className="grid w-full cursor-pointer grid-cols-[minmax(0,1fr)_minmax(60px,auto)] items-center gap-x-3 gap-y-1 border-b border-[var(--u-color-border)] px-3 py-2 text-left hover:bg-[var(--u-color-surface-hover)] focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-[var(--u-color-focus)]"
              key={entry.id}
              onClick={() => onSelect(entry)}
              title={entry.sql}
              type="button"
            >
              <span className="truncate font-mono">{entry.sql}</span>
              <span className="justify-self-end">
                <StatusBadge tone={entry.status === "success" ? "success" : "danger"}>
                  {t(entry.status === "success" ? "database.history.statusSuccess" : "database.history.statusFailed")}
                </StatusBadge>
              </span>
              <span className="flex min-w-0 items-center gap-2 text-[var(--u-color-text-muted)]">
                <span className="truncate" title={entry.connectionName}>{entry.connectionName}</span>
                <time className="shrink-0 tabular-nums" dateTime={entry.executedAt}>{formatHistoryTime(entry.executedAt, locale)}</time>
              </span>
              <span className="max-w-32 truncate text-right text-[var(--u-color-text-soft)]" title={entry.error}>
                {entry.status === "success"
                  ? t("database.history.rows", { count: entry.rowCount ?? entry.affectedRows ?? 0 })
                  : entry.error ?? t("database.history.failedFallback")}
              </span>
            </button>
          ))}
        </div>
      )}
      <ConfirmDialog
        confirmLabel={t(clear.isError ? "common.actions.retry" : "database.actions.clearHistory")}
        description={<>
          {t("database.history.clearDescription", { workspace: workspaceName })}
          {clear.isError && <span className="mt-2 block break-words text-[var(--u-color-danger)]" role="alert">
            {formatDatabaseError(clear.error, t)}
          </span>}
        </>}
        onConfirm={() => clear.mutate(workspaceId, { onSuccess: () => setConfirmOpen(false) })}
        onOpenChange={setConfirmOpen}
        open={active && confirmOpen}
        pending={clear.isPending}
        title={t("database.history.clearTitle")}
      />
    </>
  );
}

function formatHistoryTime(value: string, locale: string) {
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  const today = date.toDateString() === new Date().toDateString();
  return date.toLocaleString(locale, { timeStyle: "medium", ...(today ? {} : { dateStyle: "short" }) });
}
