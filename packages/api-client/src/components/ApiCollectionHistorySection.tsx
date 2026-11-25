import { useId, useState } from "react";
import { ChevronDown, ChevronUp } from "lucide-react";
import type { UseQueryResult } from "@tanstack/react-query";
import type { ApiHistoryItem } from "@unfour/command-client";
import { Button, SidebarRow, useI18n } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { ApiHistoryTree } from "./ApiHistoryTree";
import { SidebarEmpty } from "./ApiTreeLabels";

const expandedKey = "unfour.api.collection-history-expanded";

export function ApiCollectionHistorySection({ filtering, history, onOpenIntent }: {
  filtering: boolean;
  history: UseQueryResult<ApiHistoryItem[]>;
  onOpenIntent: (intent: ApiOpenIntent) => void;
}) {
  const { t } = useI18n();
  const contentId = useId();
  const [expanded, setExpanded] = useState(() => {
    try {
      return localStorage.getItem(expandedKey) === "true";
    } catch {
      return false;
    }
  });
  const toggleLabel = t(expanded ? "api.sidebar.collapseHistory" : "api.sidebar.expandHistory");
  const Chevron = expanded ? ChevronDown : ChevronUp;
  const items = history.data;
  const showCount = filtering && items !== undefined;

  function toggleHistory() {
    setExpanded(!expanded);
    try {
      localStorage.setItem(expandedKey, String(!expanded));
    } catch {
      // The disclosure still works when preferences cannot be saved.
    }
  }

  return (
    <section className="flex max-h-[min(220px,45%)] shrink-0 flex-col border-t border-[var(--u-color-border)]">
      <SidebarRow
        aria-controls={contentId}
        aria-expanded={expanded}
        aria-label={toggleLabel}
        aria-describedby={showCount ? `${contentId}-count` : undefined}
        className="shrink-0 cursor-pointer rounded-none px-3 focus-visible:ring-inset"
        onClick={toggleHistory}
        title={toggleLabel}
      >
        <span className="flex-1 font-semibold">{t("api.sidebar.history")}</span>
        {showCount && (
          <span className="text-[length:var(--u-font-size-caption)] tabular-nums" id={`${contentId}-count`}>
            {t("api.sidebar.historyMatches", { count: items.length })}
          </span>
        )}
        <Chevron aria-hidden="true" size={14} />
      </SidebarRow>
      <div className="min-h-0 overflow-y-auto px-2 pb-2" hidden={!expanded} id={contentId}>
        {history.isPending && <SidebarEmpty>{t("common.state.loading")}</SidebarEmpty>}
        {history.isError && (
          <div className="flex flex-wrap items-center justify-between gap-1 px-2 py-1" role="alert">
            <span className="text-[length:var(--u-font-size-caption)]">{t("api.sidebar.historyError")}</span>
            <Button disabled={history.isFetching} onClick={() => void history.refetch()} size="sm" type="button">
              {t("common.actions.retry")}
            </Button>
          </div>
        )}
        {items && items.length > 0 && <ApiHistoryTree items={items} onOpenIntent={onOpenIntent} />}
        {history.isSuccess && history.data.length === 0 && (
          <SidebarEmpty>{t(filtering ? "api.sidebar.historyNoMatches" : "api.sidebar.historyEmptyCompact")}</SidebarEmpty>
        )}
      </div>
    </section>
  );
}
