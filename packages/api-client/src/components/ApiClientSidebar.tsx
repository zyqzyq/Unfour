import { useState, type ReactNode } from "react";
import { Check, Clock, FolderOpen, Pencil, Plus, Settings2, Trash2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import {
  listApiHistory,
  type ApiHistoryItem,
  type KeyValue,
} from "@unfour/command-client";
import { Badge, Button, cn, useI18n } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { useApiEnvironments } from "../hooks/useApiEnvironments";
import { ApiCollectionTree } from "./ApiCollectionTree";
import { ApiHistoryTree } from "./ApiHistoryTree";
import { EnvironmentEditor } from "./EnvironmentEditor";

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

  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col bg-[var(--u-color-surface)]",
        !shellSlot &&
          "w-[248px] shrink-0 border-r border-[var(--u-color-border)]",
      )}
    >
      <div className="flex h-[var(--u-size-tabbar)] shrink-0 items-center gap-1 border-b border-[var(--u-color-border)] px-2">
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
          <EnvironmentsPanel workspaceId={workspaceId} />
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

function EnvironmentsPanel({ workspaceId }: { workspaceId: string }) {
  const { t } = useI18n();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const { activateMut, createMut, deleteMut, environments, updateMut } =
    useApiEnvironments(workspaceId);
  const selected = environments.find((env) => env.id === selectedId) ?? null;

  function handleCreate() {
    createMut.mutate(t("api.environment.defaultName"), {
      onSuccess: (environment) => setSelectedId(environment.id),
    });
  }

  function handleDelete(environmentId: string) {
    deleteMut.mutate(environmentId, {
      onSuccess: () => {
        if (selectedId === environmentId) {
          setSelectedId(null);
        }
      },
    });
  }

  function handleSave(name: string, variables: KeyValue[]) {
    if (!selected) {
      return;
    }
    updateMut.mutate({ id: selected.id, name, variables });
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--u-color-border)] px-2 py-1.5">
        <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {t("api.sidebar.environments")}
        </span>
        <Button
          disabled={createMut.isPending}
          onClick={handleCreate}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Plus size={13} />
          {t("api.environment.new")}
        </Button>
      </div>

      {environments.length === 0 ? (
        <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
          {t("api.environment.noneConfigured")}
        </div>
      ) : (
        <div className="shrink-0 overflow-y-auto border-b border-[var(--u-color-border)] py-1">
          {environments.map((environment) => (
            <div
              className={cn(
                "group flex items-center gap-1 px-2 py-1 text-[12px]",
                selectedId === environment.id
                  ? "bg-[var(--u-color-surface-active)]"
                  : "hover:bg-[var(--u-color-surface-hover)]",
              )}
              key={environment.id}
            >
              <button
                aria-label={
                  environment.isActive
                    ? t("api.environment.deactivate")
                    : t("api.environment.activate")
                }
                className={cn(
                  "grid h-5 w-5 shrink-0 place-items-center rounded-full border",
                  environment.isActive
                    ? "border-[var(--u-color-primary)] bg-[var(--u-color-primary)] text-[var(--u-color-primary-foreground)]"
                    : "border-[var(--u-color-border)] text-transparent hover:border-[var(--u-color-primary)]",
                )}
                onClick={() =>
                  activateMut.mutate(environment.isActive ? null : environment.id)
                }
                title={
                  environment.isActive
                    ? t("api.environment.deactivate")
                    : t("api.environment.activate")
                }
                type="button"
              >
                <Check size={11} />
              </button>
              <button
                className="flex min-w-0 flex-1 items-center gap-1.5 truncate text-left font-medium text-[var(--u-color-text)]"
                onClick={() => setSelectedId(environment.id)}
                type="button"
              >
                <span className="min-w-0 truncate">{environment.name}</span>
                {environment.isActive && (
                  <Badge className="bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)] ring-[color:color-mix(in_srgb,var(--u-color-primary)_30%,transparent)]">
                    {t("api.environment.activeBadge")}
                  </Badge>
                )}
              </button>
              <button
                aria-label={t("api.environment.edit")}
                className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] group-hover:opacity-100"
                onClick={() => setSelectedId(environment.id)}
                title={t("api.environment.edit")}
                type="button"
              >
                <Pencil size={13} />
              </button>
              <button
                aria-label={t("api.environment.delete")}
                className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-danger)] group-hover:opacity-100"
                onClick={() => handleDelete(environment.id)}
                title={t("api.environment.delete")}
                type="button"
              >
                <Trash2 size={13} />
              </button>
            </div>
          ))}
        </div>
      )}

      {selected ? (
        <div className="min-h-0 flex-1 overflow-y-auto p-3">
          <EnvironmentEditor
            environment={selected}
            onSave={handleSave}
            saveError={
              updateMut.isError
                ? updateMut.error instanceof Error
                  ? updateMut.error.message
                  : String(updateMut.error)
                : null
            }
            saving={updateMut.isPending}
          />
        </div>
      ) : (
        environments.length > 0 && (
          <div className="p-3 text-[12px] text-[var(--u-color-text-muted)]">
            {t("api.environment.selectHint")}
          </div>
        )
      )}
    </div>
  );
}
