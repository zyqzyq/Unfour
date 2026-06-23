import type { ReactNode } from "react";
import { Loader2, Settings2, X } from "lucide-react";
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

type EnvironmentTabState = {
  active: boolean;
  dirty: boolean;
  onClose: () => void;
  onSelect: () => void;
  open: boolean;
};

export function ApiRequestTabs({
  activeId,
  endControl,
  environmentTab,
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
  environmentTab?: EnvironmentTabState;
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
    <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-stretch border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]">
      <div
        aria-label={t("api.tabs.openRequests")}
        className="flex min-w-0 flex-1 items-stretch overflow-x-auto"
        role="tablist"
      >
        {tabs.map((tab, index) => {
          const active = tab.id === activeId;
          const saveState = getTabSaveState(tab);
          const visualState = requestTabVisualState(tab);
          const tabNode = (
            <div
              className={cn(
                "group relative flex min-w-[150px] max-w-[230px] items-center gap-1 border-r border-[var(--u-color-border)] px-2 text-[12px]",
                active
                  ? "bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
                  : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)]",
              )}
            >
              {active && (
                <span className="absolute inset-x-0 top-0 h-0.5 bg-[var(--u-color-primary)]" />
              )}
              <button
                aria-selected={active}
                className="flex min-w-0 flex-1 items-center gap-1.5"
                onClick={() => onSelect(tab.id)}
                role="tab"
                title={requestTabTitle(tab) + " · " + visualState}
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
                <span className="truncate">{requestTabTitle(tab)}</span>
              </button>
              <button
                aria-label={t("api.tabs.close", { title: requestTabTitle(tab) })}
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
        {environmentTab?.open && (
          <div
            className={cn(
              "group relative flex min-w-[170px] max-w-[230px] items-center gap-1 border-r border-[var(--u-color-border)] px-2 text-[12px]",
              environmentTab.active
                ? "bg-[var(--u-color-surface)] text-[var(--u-color-text)]"
                : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)]",
            )}
          >
            {environmentTab.active && (
              <span className="absolute inset-x-0 top-0 h-0.5 bg-[var(--u-color-primary)]" />
            )}
            <button
              aria-selected={environmentTab.active}
              className="flex min-w-0 flex-1 items-center gap-1.5"
              onClick={environmentTab.onSelect}
              role="tab"
              title={t("api.sidebar.environments")}
              type="button"
            >
              <Settings2 className="shrink-0 text-[var(--u-color-text-muted)]" size={13} />
              {environmentTab.dirty && (
                <span
                  aria-label={t("api.tabs.dirty")}
                  className="h-1.5 w-1.5 shrink-0 rounded-full bg-[var(--u-color-primary)]"
                />
              )}
              <span className="truncate">{t("api.sidebar.environments")}</span>
            </button>
            <button
              aria-label={t("api.tabs.close", { title: t("api.sidebar.environments") })}
              className="grid h-5 w-5 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] hover:bg-[var(--u-color-surface-hover)]"
              onClick={environmentTab.onClose}
              type="button"
            >
              <X size={12} />
            </button>
          </div>
        )}
      </div>
      <button
        aria-label={t("common.actions.newRequest")}
        className="grid h-full w-8 shrink-0 place-items-center border-r border-[var(--u-color-border)] text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]"
        onClick={onNew}
        title={t("common.actions.newRequest")}
        type="button"
      >
        +
      </button>
      {endControl && (
        <div className="flex shrink-0 items-center border-l border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
          <div className="flex shrink-0 items-center">{endControl}</div>
        </div>
      )}
    </div>
  );
}