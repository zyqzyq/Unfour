import { useEffect, useState, type ReactNode } from "react";
import { Circle, Clock, FolderOpen, MoreHorizontal, Plus, Settings2, Trash2 } from "lucide-react";
import { useQuery } from "@tanstack/react-query";
import {
  listApiHistory,
  type ApiEnvironment,
  type ApiHistoryItem,
} from "@unfour/command-client";
import {
  Button,
  ConfirmDialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  cn,
  useI18n,
} from "@unfour/ui";
import { useApiEnvironments } from "../hooks/useApiEnvironments";
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
  environmentPanelActive = false,
  onEditEnvironment,
  onNewEnvironment,
  onNewRequest,
  onOpenEnvironments,
  onOpenIntent,
  selectedEnvironmentId,
  selectedId,
  shellSlot = false,
  workspaceId,
}: {
  environmentPanelActive?: boolean;
  onEditEnvironment: (environmentId: string) => void;
  onNewEnvironment: () => void;
  onNewRequest: () => void;
  onOpenEnvironments: () => void;
  onOpenIntent: (intent: ApiOpenIntent) => void;
  selectedEnvironmentId: string | null;
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

  useEffect(() => {
    if (environmentPanelActive) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- intentional: external prop drives tab sync
      setActiveTab("environments");
    }
  }, [environmentPanelActive]);

  return (
    <div
      className={cn(
        "flex h-full min-h-0 flex-col bg-[var(--u-color-surface)]",
        !shellSlot &&
          "w-[248px] shrink-0 border-r border-[var(--u-color-border)]",
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
                "flex h-[26px] w-[26px] items-center justify-center rounded-[var(--u-radius-md)] border border-transparent text-[var(--u-color-text-muted)] transition-colors",
                active
                  ? "bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]"
                  : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
              )}
              key={tab.id}
              onClick={() => {
                if (tab.id === "environments") {
                  setActiveTab("environments");
                  onOpenEnvironments();
                  return;
                }
                setActiveTab(tab.id);
              }}
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
            items={historyQuery.data ?? []}
            onOpenIntent={onOpenIntent}
          />
        )}
        {activeTab === "environments" && (
          <EnvironmentsPanel
            onEditEnvironment={onEditEnvironment}
            onNewEnvironment={onNewEnvironment}
            selectedEnvironmentId={selectedEnvironmentId}
            workspaceId={workspaceId}
          />
        )}
      </div>
    </div>
  );
}

function EnvironmentsPanel({
  onEditEnvironment,
  onNewEnvironment,
  selectedEnvironmentId,
  workspaceId,
}: {
  onEditEnvironment: (environmentId: string) => void;
  onNewEnvironment: () => void;
  selectedEnvironmentId: string | null;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const { activateMut, deleteMut, environments, isLoading } = useApiEnvironments(workspaceId);
  const [deleteTarget, setDeleteTarget] = useState<ApiEnvironment | null>(null);

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center justify-between gap-2 px-2 py-2">
        <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {t("api.sidebar.environments")}
        </span>
        <Button onClick={onNewEnvironment} size="sm" type="button" variant="ghost">
          <Plus size={13} />
          {t("api.environment.new")}
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {isLoading ? (
          <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
            {t("common.state.loading")}
          </div>
        ) : environments.length === 0 ? (
          <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
            {t("api.environment.noneConfigured")}
          </div>
        ) : (
          <div className="space-y-1">
            {environments.map((environment) => (
              <EnvironmentRow
                environment={environment}
                key={environment.id}
                onActivate={() =>
                  activateMut.mutate(environment.isActive ? null : environment.id)
                }
                onDelete={() => setDeleteTarget(environment)}
                onSelect={() => onEditEnvironment(environment.id)}
                selected={selectedEnvironmentId === environment.id}
              />
            ))}
          </div>
        )}
      </div>
      <ConfirmDialog
        confirmLabel={t("api.environment.delete")}
        description={
          deleteTarget
            ? t("api.environment.deleteConfirm", { name: deleteTarget.name })
            : undefined
        }
        onConfirm={() => {
          if (deleteTarget) {
            deleteMut.mutate(deleteTarget.id, {
              onSuccess: () => setDeleteTarget(null),
            });
          }
        }}
        onOpenChange={(open) => {
          if (!open) {
            setDeleteTarget(null);
          }
        }}
        open={Boolean(deleteTarget)}
        pending={deleteMut.isPending}
        title={t("api.environment.delete")}
      />
    </div>
  );
}

function EnvironmentRow({
  environment,
  onActivate,
  onDelete,
  onSelect,
  selected,
}: {
  environment: ApiEnvironment;
  onActivate: () => void;
  onDelete: () => void;
  onSelect: () => void;
  selected: boolean;
}) {
  const { t } = useI18n();

  return (
    <div
      className={cn(
        "group flex w-full min-w-0 items-center gap-1.5 rounded-[var(--u-radius-md)] px-2 py-1.5 text-left text-[12px] transition-colors",
        selected
          ? "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]"
          : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
      )}
    >
      <button
        aria-label={environment.name}
        className="min-w-0 flex-1 truncate font-medium"
        onClick={onSelect}
        type="button"
      >
        {environment.name}
      </button>
      <button
        aria-label={
          environment.isActive
            ? t("api.environment.deactivate")
            : t("api.environment.activate")
        }
        className={cn(
          "inline-flex shrink-0 items-center justify-center rounded-full transition-colors",
          environment.isActive
            ? "text-[var(--u-color-primary)]"
            : "text-[var(--u-color-text-muted)] opacity-0 group-hover:opacity-100",
        )}
        onClick={onActivate}
        title={
          environment.isActive
            ? t("api.environment.deactivate")
            : t("api.environment.activate")
        }
        type="button"
      >
        <Circle
          fill={environment.isActive ? "currentColor" : "none"}
          size={10}
          strokeWidth={2}
        />
      </button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <IconButton
            aria-label={t("api.environment.actions")}
            className="shrink-0 opacity-0 transition-opacity group-hover:opacity-100"
            label={t("api.environment.actions")}
            size="compact"
          >
            <MoreHorizontal size={13} />
          </IconButton>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem
            className="text-[var(--u-color-danger)]"
            onSelect={onDelete}
          >
            <Trash2 size={13} />
            {t("api.environment.delete")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
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
