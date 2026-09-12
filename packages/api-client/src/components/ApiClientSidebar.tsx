import { useState, type ReactNode } from "react";
import { Clock, FolderOpen } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { listApiHistory, type ApiHistoryItem } from "@unfour/command-client";
import { Button, cn, EmptyState, ErrorState, LoadingState, useI18n } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { ApiCollectionTree } from "./ApiCollectionTree";
import { ApiHistoryTree } from "./ApiHistoryTree";

type SidebarTab = "collections" | "history";

const sidebarTabs: Array<{ id: SidebarTab; icon: ReactNode; labelKey: string }> = [
  { id: "collections", icon: <FolderOpen size={14} />, labelKey: "api.sidebar.collections" },
  { id: "history", icon: <Clock size={14} />, labelKey: "api.sidebar.history" },
];

export function ApiClientSidebar({
  onNewRequest,
  onOpenIntent,
  selectedId,
  shellSlot = false,
  workspaceId,
}: {
  onNewRequest: () => void;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  selectedId: string | null;
  shellSlot?: boolean;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const [activeTab, setActiveTab] = useState<SidebarTab>("collections");
  const historyQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-history", workspaceId],
    queryFn: () => listApiHistory(workspaceId),
  });

  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col bg-[var(--u-color-surface)]",
        !shellSlot && "w-[248px] shrink-0 border-r border-[var(--u-color-border)]",
      )}
    >
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-center gap-1 border-b border-[var(--u-color-border)] px-2">
        {sidebarTabs.map((tab) => {
          const active = activeTab === tab.id;
          return (
            <button
              aria-label={t(tab.labelKey)}
              aria-pressed={active}
              className={cn(
                "flex h-[26px] w-[26px] cursor-pointer items-center justify-center rounded-[var(--u-radius-md)] border border-transparent text-[var(--u-color-text-muted)] transition-colors",
                active
                  ? "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]"
                  : "hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
              )}
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              title={t(tab.labelKey)}
              type="button"
            >
              {tab.icon}
            </button>
          );
        })}
      </div>

      <div className="min-h-0 flex-1 overflow-hidden">
        {activeTab === "collections" && (
          <ApiCollectionTree
            active
            collapsed={false}
            onOpenClient={onNewRequest}
            onOpenIntent={onOpenIntent}
            selectedId={selectedId}
            workspaceId={workspaceId}
          />
        )}
        {activeTab === "history" && (
          <HistoryPanel
            error={historyQuery.error}
            isLoading={historyQuery.isLoading}
            items={historyQuery.data}
            onOpenIntent={onOpenIntent}
            onRetry={() => {
              void historyQuery.refetch();
            }}
          />
        )}
      </div>
    </div>
  );
}

function HistoryPanel({
  error,
  isLoading,
  items,
  onOpenIntent,
  onRetry,
}: {
  error: unknown;
  isLoading: boolean;
  items: ApiHistoryItem[] | undefined;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  onRetry: () => void;
}) {
  const { t } = useI18n();
  const loadedItems = items ?? [];
  const hasItems = loadedItems.length > 0;

  if (isLoading && !hasItems) {
    return <LoadingState className="m-2 min-h-[120px]" />;
  }

  if (error && !hasItems) {
    return (
      <ErrorState className="m-2 min-h-[120px]">
        <div className="space-y-2">
          <div>{t("api.sidebar.historyError")}</div>
          <Button onClick={onRetry} size="sm" type="button">
            {t("common.actions.retry")}
          </Button>
        </div>
      </ErrorState>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {error ? (
        <ErrorState className="m-2 min-h-[48px]">
          <div className="flex flex-wrap items-center justify-center gap-2">
            <span>{t("api.sidebar.historyError")}</span>
            <Button onClick={onRetry} size="sm" type="button">
              {t("common.actions.retry")}
            </Button>
          </div>
        </ErrorState>
      ) : null}
      {hasItems ? (
        <div className="min-h-0 flex-1 overflow-y-auto p-2">
          <ApiHistoryTree items={loadedItems} onOpenIntent={onOpenIntent} />
        </div>
      ) : (
        <EmptyState className="m-2 min-h-[120px]">{t("api.sidebar.historyEmpty")}</EmptyState>
      )}
    </div>
  );
}
