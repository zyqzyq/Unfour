import { useEffect, useState, type ReactNode } from "react";
import { Check, Clock, FolderOpen, Plus, Settings2, Trash2 } from "lucide-react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  activateApiEnvironment,
  createApiEnvironment,
  deleteApiEnvironment,
  listApiEnvironments,
  listApiHistory,
  updateApiEnvironment,
  type ApiEnvironment,
  type ApiHistoryItem,
  type KeyValue,
} from "@unfour/command-client";
import { Button, Input, cn, useI18n } from "@unfour/ui";
import type { ApiOpenIntent } from "../model/types";
import { ApiCollectionTree } from "./ApiCollectionTree";
import { ApiHistoryTree } from "./ApiHistoryTree";
import { EnvironmentHints, KeyValueEditor } from "./KeyValueEditor";

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
  const queryClient = useQueryClient();
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [nameDraft, setNameDraft] = useState("");
  const [variablesDraft, setVariablesDraft] = useState<KeyValue[]>([]);

  const environmentsQuery = useQuery({
    enabled: Boolean(workspaceId),
    queryKey: ["api-environments", workspaceId],
    queryFn: () => listApiEnvironments(workspaceId),
  });
  const environments = environmentsQuery.data ?? [];
  const selected = environments.find((env) => env.id === selectedId) ?? null;

  useEffect(() => {
    if (!selected) {
      setNameDraft("");
      setVariablesDraft([]);
      return;
    }
    setNameDraft(selected.name);
    setVariablesDraft(selected.variables);
  }, [workspaceId, selected?.id, selected?.updatedAt]);

  const invalidate = () =>
    queryClient.invalidateQueries({ queryKey: ["api-environments", workspaceId] });

  const createMutation = useMutation({
    mutationFn: () => createApiEnvironment(workspaceId, t("api.environment.defaultName")),
    onSuccess: (environment) => {
      invalidate();
      setSelectedId(environment.id);
    },
  });
  const saveMutation = useMutation({
    mutationFn: (environment: ApiEnvironment) =>
      updateApiEnvironment(workspaceId, environment.id, nameDraft, variablesDraft),
    onSuccess: invalidate,
  });
  const deleteMutation = useMutation({
    mutationFn: (environmentId: string) =>
      deleteApiEnvironment(workspaceId, environmentId),
    onSuccess: (_environments, environmentId) => {
      invalidate();
      if (selectedId === environmentId) {
        setSelectedId(null);
      }
    },
  });
  const activateMutation = useMutation({
    mutationFn: (environmentId: string | null) =>
      activateApiEnvironment(workspaceId, environmentId),
    onSuccess: invalidate,
  });

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex shrink-0 items-center justify-between gap-2 border-b border-[var(--u-color-border)] px-2 py-1.5">
        <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {t("api.sidebar.environments")}
        </span>
        <Button
          disabled={createMutation.isPending}
          onClick={() => createMutation.mutate()}
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
                aria-label={t("api.environment.activate")}
                className={cn(
                  "grid h-5 w-5 shrink-0 place-items-center rounded-full border",
                  environment.isActive
                    ? "border-[var(--u-color-primary)] bg-[var(--u-color-primary)] text-[var(--u-color-primary-foreground)]"
                    : "border-[var(--u-color-border)] text-transparent hover:border-[var(--u-color-primary)]",
                )}
                onClick={() =>
                  activateMutation.mutate(environment.isActive ? null : environment.id)
                }
                title={t("api.environment.activate")}
                type="button"
              >
                <Check size={11} />
              </button>
              <button
                className="min-w-0 flex-1 truncate text-left font-medium text-[var(--u-color-text)]"
                onClick={() => setSelectedId(environment.id)}
                type="button"
              >
                {environment.name}
              </button>
              <button
                aria-label={t("api.environment.delete")}
                className="grid h-6 w-6 shrink-0 place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-danger)] group-hover:opacity-100"
                onClick={() => deleteMutation.mutate(environment.id)}
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
        <div className="min-h-0 flex-1 space-y-2 overflow-y-auto p-3">
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            {t("api.environment.nameLabel")}
            <Input
              onChange={(event) => setNameDraft(event.target.value)}
              value={nameDraft}
            />
          </label>
          <KeyValueEditor
            items={variablesDraft}
            maskSensitiveValues
            onChange={setVariablesDraft}
            title={t("api.environment.variablesLabel")}
          />
          <EnvironmentHints variables={variablesDraft} />
          <div className="flex justify-end">
            <Button
              disabled={saveMutation.isPending || !nameDraft.trim()}
              onClick={() => saveMutation.mutate(selected)}
              size="sm"
              type="button"
            >
              {saveMutation.isPending
                ? t("api.actions.saving")
                : t("api.environment.save")}
            </Button>
          </div>
          {saveMutation.isError && (
            <div className="text-[12px] text-[var(--u-color-danger)]">
              {saveMutation.error instanceof Error
                ? saveMutation.error.message
                : String(saveMutation.error)}
            </div>
          )}
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
