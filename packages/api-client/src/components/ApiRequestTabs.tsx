import { Loader2, X } from "lucide-react";
import { cn } from "@unfour/ui";
import {
  getTabSaveState,
  methodBadgeLabel,
  methodToneClass,
  requestTabTitle,
  requestTabVisualState,
  type ApiRequestTab,
} from "../model/request-tabs";

export function ApiRequestTabs({
  activeId,
  onClose,
  onNew,
  onSelect,
  tabs,
}: {
  activeId: string | null;
  onClose: (tab: ApiRequestTab) => void;
  onNew: () => void;
  onSelect: (tabId: string) => void;
  tabs: ApiRequestTab[];
}) {
  return (
    <div
      aria-label="Open API requests"
      className="flex h-[var(--u-size-tabbar)] shrink-0 items-stretch overflow-x-auto border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]"
      role="tablist"
    >
      {tabs.map((tab) => {
        const active = tab.id === activeId;
        const saveState = getTabSaveState(tab);
        const visualState = requestTabVisualState(tab);
        return (
          <div
            className={cn(
              "group relative flex min-w-[150px] max-w-[230px] items-center gap-1 border-r border-[var(--u-color-border)] px-2 text-[12px]",
              active
                ? "bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
                : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)]",
            )}
            key={tab.id}
          >
            {active && (
              <span className="absolute inset-x-0 top-0 h-0.5 bg-[var(--u-color-primary)]" />
            )}
            <button
              aria-selected={active}
              className="flex min-w-0 flex-1 items-center gap-1.5"
              onClick={() => onSelect(tab.id)}
              role="tab"
              title={`${requestTabTitle(tab)} · ${visualState}`}
              type="button"
            >
              <span
                className={cn(
                  "w-9 shrink-0 text-left text-[10px] font-bold uppercase tabular-nums",
                  methodToneClass(tab.draft.method),
                )}
              >
                {methodBadgeLabel(tab.draft.method)}
              </span>
              {(saveState === "dirty" || saveState === "unsaved") && (
                <span
                  aria-label={saveState}
                  className="shrink-0 text-[13px] font-semibold text-[var(--u-color-primary)]"
                >
                  *
                </span>
              )}
              {tab.sending && (
                <Loader2
                  aria-label="sending"
                  className="shrink-0 animate-spin text-[var(--u-color-primary)]"
                  size={12}
                />
              )}
              <span className="truncate">{requestTabTitle(tab)}</span>
            </button>
            <button
              aria-label={`Close ${requestTabTitle(tab)}`}
              className="grid h-5 w-5 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
              onClick={() => onClose(tab)}
              type="button"
            >
              <X size={12} />
            </button>
          </div>
        );
      })}
      <button
        aria-label="New request"
        className="grid w-10 shrink-0 place-items-center border-r border-[var(--u-color-border)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
        onClick={onNew}
        title="New request"
        type="button"
      >
        +
      </button>
    </div>
  );
}
