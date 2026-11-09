import type { ReactNode } from "react";
import { Loader2, Plus, X } from "lucide-react";
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuTrigger,
  cn,
  useI18n,
} from "@unfour/ui";
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
  endControl,
  onClose,
  onCloseAll,
  onCloseLeft,
  onCloseRight,
  onCloseSaved,
  onNew,
  onSelect,
  tabs,
}: {
  activeId: string | null;
  endControl?: ReactNode;
  onClose: (tab: ApiRequestTab) => void;
  onCloseAll: () => void;
  onCloseLeft: (tab: ApiRequestTab) => void;
  onCloseRight: (tab: ApiRequestTab) => void;
  onCloseSaved: () => void;
  onNew: () => void;
  onSelect: (tabId: string) => void;
  tabs: ApiRequestTab[];
}) {
  const { t } = useI18n();
  const savedTabCount = tabs.filter((tab) => getTabSaveState(tab) === "saved").length;

  return (
    <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-end gap-1 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
      <div
        aria-label={t("api.tabs.openRequests")}
        className="flex min-w-0 shrink translate-y-px items-end gap-1 overflow-x-auto"
        role="tablist"
      >
        {tabs.map((tab, index) => {
          const active = tab.id === activeId;
          const saveState = getTabSaveState(tab);
          const visualState = requestTabVisualState(tab);
          const title = requestTabTitle(tab, t("api.request.untitled"));
          const tabNode = (
            <div
              className={cn(
                "group relative flex h-9 min-w-[150px] max-w-[230px] shrink-0 items-center gap-2 rounded-t-[var(--u-radius-sm)] border border-transparent px-2.5 text-[12px] transition-colors duration-150",
                active
                  ? "border-[var(--u-color-border)] border-b-[var(--u-color-canvas)] bg-[var(--u-color-canvas)] font-medium text-[var(--u-color-text)]"
                  : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)]",
              )}
            >
              {active && (
                <span className="absolute inset-x-2 top-0 h-0.5 rounded-full bg-[var(--u-color-primary)]" />
              )}
              <button
                aria-selected={active}
                className="flex h-full min-w-0 flex-1 items-center gap-1.5 rounded-[var(--u-radius-sm)] focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[-2px] focus-visible:outline-[var(--u-color-focus)]"
                onClick={() => onSelect(tab.id)}
                role="tab"
                title={title + " · " + t("api.tabs.state." + visualState)}
                type="button"
              >
                <span
                  className={cn(
                    "shrink-0 font-mono text-[10px] font-bold uppercase",
                    methodToneClass(tab.draft.method),
                  )}
                >
                  {methodBadgeLabel(tab.draft.method)}
                </span>
                <span className="min-w-0 flex-1 truncate text-left">{title}</span>
                {(saveState === "dirty" || saveState === "unsaved") && (
                  <span
                    aria-label={t("api.tabs." + saveState)}
                    className="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--u-color-primary)]"
                  />
                )}
                {tab.sending && (
                  <Loader2
                    aria-label={t("api.actions.sending")}
                    className="shrink-0 animate-spin text-[var(--u-color-primary)]"
                    size={12}
                  />
                )}
              </button>
              <button
                aria-label={t("api.tabs.close", { title })}
                className="grid h-5 w-5 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
                onClick={() => onClose(tab)}
                type="button"
              >
                <X size={12} />
              </button>
            </div>
          );
          return (
            <ContextMenu key={tab.id}>
              <ContextMenuTrigger asChild>{tabNode}</ContextMenuTrigger>
              <ContextMenuContent>
                <ContextMenuItem onSelect={() => onClose(tab)}>
                  {t("api.tabs.closeTab")}
                </ContextMenuItem>
                <ContextMenuItem onSelect={onCloseAll}>
                  {t("api.tabs.closeAll")}
                </ContextMenuItem>
                <ContextMenuItem disabled={!savedTabCount} onSelect={onCloseSaved}>
                  {t("api.tabs.closeSaved")}
                </ContextMenuItem>
                <ContextMenuItem disabled={index === 0} onSelect={() => onCloseLeft(tab)}>
                  {t("api.tabs.closeLeft")}
                </ContextMenuItem>
                <ContextMenuItem
                  disabled={index === tabs.length - 1}
                  onSelect={() => onCloseRight(tab)}
                >
                  {t("api.tabs.closeRight")}
                </ContextMenuItem>
              </ContextMenuContent>
            </ContextMenu>
          );
        })}
      </div>
      <button
        aria-label={t("common.actions.newRequest")}
        className="grid h-7 w-7 shrink-0 self-center place-items-center rounded-[var(--u-radius-md)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
        onClick={onNew}
        title={t("common.actions.newRequest")}
        type="button"
      >
        <Plus size={14} />
      </button>
      <div className="min-w-0 flex-1" />
      {endControl && (
        <div className="flex shrink-0 items-center self-center pl-2">
          <div className="flex shrink-0 items-center">{endControl}</div>
        </div>
      )}
    </div>
  );
}
