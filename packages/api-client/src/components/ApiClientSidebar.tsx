import { useState, type ReactNode } from "react";
import { Clock, FolderOpen, Settings2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import {
  getWorkspaceEnvironment,
  listApiHistory,
  type ApiHistoryItem,
  type KeyValue,
} from "@unfour/command-client";
import { cn, useI18n } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { ApiCollectionTree } from "./ApiCollectionTree";
import { ApiHistoryTree } from "./ApiHistoryTree";

type SidebarTab = "collections" | "history" | "environments";

const sidebarTabs: Array<{ id: SidebarTab; icon: ReactNode; labelKey: string }> = [
  { id: "collections", icon: <FolderOpen size={14} />, labelKey: "api.sidebar.collections" },
  { id: "history", icon: <Clock size={14} />, labelKey: "api.sidebar.history" },
  { id: "environments", icon: <Settings2 size={14} />, labelKey: "api.sidebar.environments" },
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
  const environmentQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["workspace-environment", workspaceId],
    queryFn: () => getWorkspaceEnvironment(workspaceId),
  });

  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col bg-[var(--u-color-surface)]",
        !shellSlot &&
          "w-[248px] shrink-0 border-r border-[var(--u-color-border)]",
      )}
    >
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] px-2">
        <h2 className="min-w-0 flex-1 truncate text-[11px] font-bold uppercase tracking-[0.07em] text-[var(--u-color-text-muted)]">
          {t("api.sidebar.restClient")}
        </h2>
        <div className="flex shrink-0 items-center gap-1">
          {sidebarTabs.map((tab) => (
            <button
              aria-label={t(tab.labelKey)}
              aria-pressed={activeTab === tab.id}
              className={cn(
                "flex h-[26px] w-[26px] items-center justify-center rounded-[var(--u-radius-md)] border border-transparent text-[var(--u-color-text-muted)] transition-colors",
                activeTab === tab.id
                  ? "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]"
                  : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
              )}
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              title={t(tab.labelKey)}
              type="button"
            >
              {tab.icon}
            </button>
          ))}
        </div>
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
            items={historyQuery.data ?? []}
            onOpenIntent={onOpenIntent}
          />
        )}
        {activeTab === "environments" && (
          <EnvironmentsPanel variables={environmentQuery.data?.variables ?? []} />
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

function EnvironmentsPanel({ variables }: { variables: KeyValue[] }) {
  const { t } = useI18n();
  const enabledVariables = variables.filter((variable) => variable.enabled);

  if (!enabledVariables.length) {
    return (
      <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
        {t("api.environment.noneConfigured")}
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 overflow-y-auto p-3 text-[12px]">
      <div className="mb-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
        {t("api.environment.workspaceVariables", { count: enabledVariables.length })}
      </div>
      <div className="space-y-1">
        {enabledVariables.map((variable) => (
          <div
            className="flex items-center gap-2 rounded-[var(--u-radius-sm)] px-1.5 py-1 hover:bg-[var(--u-color-surface-hover)]"
            key={variable.key}
          >
            <span className="truncate font-medium">{variable.key}</span>
            <span className="min-w-0 flex-1 truncate font-mono text-[var(--u-color-text-muted)]">
              {variable.value}
            </span>
          </div>
        ))}
      </div>
    </div>
  );
}
