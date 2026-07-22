import { useState, type ReactNode } from "react";
import { Clock, FolderOpen } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import { listApiHistory, type ApiHistoryItem } from "@unfour/command-client";
import { cn, useI18n } from "@unfour/ui";
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
          <HistoryPanel items={historyQuery.data ?? []} onOpenIntent={onOpenIntent} />
        )}
      </div>
    </div>
  );
}

function HistoryPanel({
  items,
  onOpenIntent,
}: {
  items: ApiHistoryItem[];
  onOpenIntent: (intent: ApiOpenIntent) => void;
}) {
  const { t } = useI18n();
  if (!items.length) {
    return (
      <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
        {t("api.sidebar.historyEmpty")}
      </div>
    );
  }
  return (
    <div className="h-full min-h-0 overflow-y-auto p-2">
      <ApiHistoryTree items={items} onOpenIntent={onOpenIntent} />
    </div>
  );
}
