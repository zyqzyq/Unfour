import { useState } from "react";
import { Clock, FolderOpen, Settings2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import {
  getWorkspaceEnvironment,
  listApiHistory,
  type ApiHistoryItem,
  type KeyValue,
} from "@unfour/command-client";
import { cn } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { ApiCollectionTree } from "./ApiCollectionTree";
import { ApiHistoryTree } from "./ApiHistoryTree";

type SidebarTab = "collections" | "history" | "environments";

const sidebarTabs: Array<{ id: SidebarTab; icon: React.ReactNode; label: string }> = [
  { id: "collections", icon: <FolderOpen size={14} />, label: "Collections" },
  { id: "history", icon: <Clock size={14} />, label: "History" },
  { id: "environments", icon: <Settings2 size={14} />, label: "Environments" },
];

export function ApiClientSidebar({
  onNewRequest,
  onOpenIntent,
  selectedId,
  workspaceId,
}: {
  onNewRequest: () => void;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  selectedId: string | null;
  workspaceId: string;
}) {
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
    <div className="flex w-[248px] shrink-0 flex-col border-r border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)]">
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-end border-b border-[var(--u-color-border)] px-1">
        {sidebarTabs.map((tab) => (
          <button
            className={cn(
              "flex h-[29px] items-center gap-1.5 rounded-t-[var(--u-radius-sm)] border border-transparent px-2 text-[12px] font-medium transition-colors",
              activeTab === tab.id
                ? "border-[var(--u-color-border)] border-b-[var(--u-color-surface-subtle)] bg-[var(--u-color-surface-subtle)] text-[var(--u-color-text)]"
                : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
            )}
            key={tab.id}
            onClick={() => setActiveTab(tab.id)}
            type="button"
          >
            {tab.icon}
            <span>{tab.label}</span>
          </button>
        ))}
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
  if (!items.length) {
    return (
      <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
        Send a request to build history.
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
  const enabledVariables = variables.filter((variable) => variable.enabled);

  if (!enabledVariables.length) {
    return (
      <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
        No environment variables configured. Use the Auth tab in the request
        editor to add variables.
      </div>
    );
  }

  return (
    <div className="h-full min-h-0 overflow-y-auto p-3 text-[12px]">
      <div className="mb-2 text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
        Workspace variables ({enabledVariables.length})
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
