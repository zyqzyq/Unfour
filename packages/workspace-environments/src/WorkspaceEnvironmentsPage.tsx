import { useEffect, useState } from "react";
import { Circle, MoreHorizontal, Plus, Settings2, X } from "lucide-react";
import type { WorkspaceEnvironment } from "@unfour/command-client";
import {
  Button,
  cn,
  ConfirmDialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  useFeedbackErrorHandler,
  useI18n,
} from "@unfour/ui";
import {
  WorkspaceEnvironmentEditor,
  type EnvironmentEditorMode,
} from "./WorkspaceEnvironmentEditor";
import { nextEnvironmentName } from "./environment-utils";
import { useWorkspaceEnvironments } from "./hooks/useWorkspaceEnvironments";
import { useWorkspaceVariables } from "./hooks/useWorkspaceVariables";

type ManagerSelection =
  | { kind: "workspace" }
  | { environmentId: string; kind: "environment" }
  | { kind: "new" };

type PendingAction = { kind: "close" } | { kind: "select"; selection: ManagerSelection };

export function WorkspaceEnvironmentsPage({
  initialEnvironmentId = null,
  onClose,
  onDirtyChange,
  workspaceId,
}: {
  initialEnvironmentId?: string | null;
  onClose: () => void;
  onDirtyChange?: (dirty: boolean) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const {
    createMut,
    deleteMut,
    environments,
    isLoading,
    updateMut,
  } = useWorkspaceEnvironments(workspaceId);
  const {
    isLoading: workspaceVariablesLoading,
    replaceMut,
    variables: workspaceVariables,
  } = useWorkspaceVariables(workspaceId);
  const [selection, setSelection] = useState<ManagerSelection>(() =>
    initialEnvironmentId
      ? { environmentId: initialEnvironmentId, kind: "environment" }
      : { kind: "workspace" },
  );
  const [modeRevision, setModeRevision] = useState(1);
  const [dirty, setDirty] = useState(false);
  const [pendingAction, setPendingAction] = useState<PendingAction | null>(null);

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    return () => onDirtyChange?.(false);
  }, [onDirtyChange]);

  function applySelection(next: ManagerSelection) {
    setSelection(next);
    setModeRevision((current) => current + 1);
  }

  function requestSelection(next: ManagerSelection) {
    if (sameSelection(selection, next)) return;
    if (dirty) {
      setPendingAction({ kind: "select", selection: next });
      return;
    }
    applySelection(next);
  }

  function requestClose() {
    if (dirty) {
      setPendingAction({ kind: "close" });
      return;
    }
    onClose();
  }

  function discardAndContinue() {
    const pending = pendingAction;
    setPendingAction(null);
    if (!pending) return;
    if (pending.kind === "close") {
      onClose();
      return;
    }
    applySelection(pending.selection);
  }

  const mode: EnvironmentEditorMode = selection;
  return (
    <section className="flex h-full min-h-0 min-w-0 flex-col bg-[var(--u-color-bg)]">
      <header className="flex h-[var(--u-size-section-toolbar)] shrink-0 items-center justify-between gap-3 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-3">
        <div className="min-w-0">
          <h1 className="truncate text-[13px] font-semibold text-[var(--u-color-text)]">
            {t("variables.managerTitle")}
          </h1>
        </div>
        <IconButton label={t("variables.closeManager")} onClick={requestClose}>
          <X size={14} />
        </IconButton>
      </header>
      <div className="flex min-h-0 flex-1">
        <EnvironmentList
          creating={createMut.isPending || updateMut.isPending}
          deleting={deleteMut.isPending}
          environments={environments}
          isLoading={isLoading}
          onCreate={() => requestSelection({ kind: "new" })}
          onDelete={async (environmentId) => {
            await deleteMut.mutateAsync(environmentId);
            if (
              selection.kind === "environment" &&
              selection.environmentId === environmentId
            ) {
              applySelection({ kind: "workspace" });
            }
          }}
          onDuplicate={async (environment) => {
            const name = nextEnvironmentName(environment.name, environments);
            const created = await createMut.mutateAsync(name);
            await updateMut.mutateAsync({
              id: created.id,
              name,
              variables: environment.variables.map((variable) => ({
                id: null,
                key: variable.key,
                value: variable.value,
                isSecret: variable.isSecret,
                isEnabled: variable.isEnabled,
                description: variable.description,
                sortOrder: variable.sortOrder,
              })),
            });
          }}
          onSelectEnvironment={(environmentId) =>
            requestSelection({ environmentId, kind: "environment" })
          }
          onSelectWorkspace={() => requestSelection({ kind: "workspace" })}
          selection={selection}
        />
        <WorkspaceEnvironmentEditor
          environments={environments}
          environmentsLoading={isLoading}
          mode={mode}
          modeRevision={modeRevision}
          onCreate={(name) => createMut.mutateAsync(name)}
          onDirtyChange={setDirty}
          onEnvironmentCreated={(environmentId) =>
            applySelection({ environmentId, kind: "environment" })
          }
          onReplaceWorkspaceVariables={(variables) => replaceMut.mutateAsync(variables)}
          onUpdate={(input) => updateMut.mutateAsync(input)}
          saving={createMut.isPending || updateMut.isPending || replaceMut.isPending}
          workspaceVariables={workspaceVariables}
          workspaceVariablesLoading={workspaceVariablesLoading}
        />
      </div>
      <ConfirmDialog
        confirmLabel={t("variables.discard")}
        description={t("variables.discardChangesDescription")}
        onConfirm={discardAndContinue}
        onOpenChange={(open) => !open && setPendingAction(null)}
        open={pendingAction !== null}
        title={t("variables.discardChangesTitle")}
      />
    </section>
  );
}

function EnvironmentList({
  creating,
  deleting,
  environments,
  isLoading,
  onCreate,
  onDelete,
  onDuplicate,
  onSelectEnvironment,
  onSelectWorkspace,
  selection,
}: {
  creating: boolean;
  deleting: boolean;
  environments: WorkspaceEnvironment[];
  isLoading: boolean;
  onCreate: () => void;
  onDelete: (environmentId: string) => Promise<void>;
  onDuplicate: (environment: WorkspaceEnvironment) => Promise<void>;
  onSelectEnvironment: (environmentId: string) => void;
  onSelectWorkspace: () => void;
  selection: ManagerSelection;
}) {
  const { t } = useI18n();
  const handleError = useFeedbackErrorHandler();
  const [deleteTarget, setDeleteTarget] = useState<WorkspaceEnvironment | null>(null);

  return (
    <aside className="flex w-[248px] shrink-0 flex-col border-r border-[var(--u-color-border)] bg-[var(--u-color-surface)]">
      <div className="shrink-0 px-2 pt-3">
        <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {t("variables.workspaceGroup")}
        </span>
        <button
          className={cn(
            "mt-1 flex w-full cursor-pointer items-center gap-2 rounded-[var(--u-radius-md)] px-2 py-1.5 text-left text-[12px] font-medium transition-colors",
            selection.kind === "workspace"
              ? "bg-[var(--u-color-surface-active)] text-[var(--u-color-text)]"
              : "text-[var(--u-color-text-muted)] hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)]",
          )}
          onClick={onSelectWorkspace}
          type="button"
        >
          <Settings2 size={13} />
          <span className="truncate">{t("variables.workspaceVariables")}</span>
        </button>
      </div>
      <div className="flex shrink-0 items-center justify-between gap-2 px-2 pt-3 pb-1">
        <span className="text-[11px] font-semibold uppercase text-[var(--u-color-text-soft)]">
          {t("variables.environmentsGroup")}
        </span>
        <Button
          disabled={creating}
          onClick={onCreate}
          size="sm"
          type="button"
          variant="ghost"
        >
          <Plus size={13} />
          {t("variables.new")}
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 pb-2">
        {isLoading ? (
          <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
            {t("common.state.loading")}
          </div>
        ) : environments.length === 0 ? (
          <div className="px-2 py-1.5 text-[12px] text-[var(--u-color-text-muted)]">
            {t("variables.noneConfigured")}
          </div>
        ) : (
          <div className="space-y-1">
            {environments.map((environment) => (
              <EnvironmentRow
                environment={environment}
                key={environment.id}
                onDelete={() => setDeleteTarget(environment)}
                onDuplicate={() => {
                  void onDuplicate(environment).catch((error) =>
                    handleError(error, { key: "feedback.api.environmentDuplicateFailed" }),
                  );
                }}
                onSelect={() => onSelectEnvironment(environment.id)}
                selected={
                  selection.kind === "environment" &&
                  selection.environmentId === environment.id
                }
              />
            ))}
          </div>
        )}
      </div>
      <ConfirmDialog
        confirmLabel={t("common.actions.delete")}
        description={
          deleteTarget
            ? t("variables.deleteConfirm", { name: deleteTarget.name })
            : ""
        }
        onConfirm={() => {
          if (!deleteTarget) return;
          void onDelete(deleteTarget.id)
            .then(() => setDeleteTarget(null))
            .catch(() => undefined);
        }}
        onOpenChange={(open) => !open && setDeleteTarget(null)}
        open={deleteTarget !== null}
        pending={deleting}
        title={t("variables.deleteEnvironment")}
      />
    </aside>
  );
}

function EnvironmentRow({
  environment,
  onDelete,
  onDuplicate,
  onSelect,
  selected,
}: {
  environment: WorkspaceEnvironment;
  onDelete: () => void;
  onDuplicate: () => void;
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
      <span
        aria-label={environment.isActive ? t("variables.active") : undefined}
        className={cn(
          "inline-flex shrink-0 items-center justify-center rounded-full",
          environment.isActive ? "text-[var(--u-color-primary)]" : "text-transparent",
        )}
        title={environment.isActive ? t("variables.active") : undefined}
      >
        <Circle fill={environment.isActive ? "currentColor" : "none"} size={10} />
      </span>
      <button
        aria-label={environment.name}
        className="min-w-0 flex-1 cursor-pointer truncate text-left font-medium"
        onClick={onSelect}
        type="button"
      >
        {environment.name}
      </button>
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <button
            aria-label={t("variables.environmentActions", { name: environment.name })}
            className="grid h-5 w-5 shrink-0 cursor-pointer place-items-center rounded-[var(--u-radius-sm)] text-[var(--u-color-text-soft)] opacity-0 hover:bg-[var(--u-color-surface-hover)] hover:text-[var(--u-color-text)] focus-visible:opacity-100 group-hover:opacity-100 data-[state=open]:opacity-100"
            type="button"
          >
            <MoreHorizontal size={13} />
          </button>
        </DropdownMenuTrigger>
        <DropdownMenuContent align="end">
          <DropdownMenuItem onSelect={onDuplicate}>
            {t("variables.duplicate")}
          </DropdownMenuItem>
          <DropdownMenuItem className="text-[var(--u-color-danger)]" onSelect={onDelete}>
            {t("variables.deleteEnvironment")}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}

function sameSelection(left: ManagerSelection, right: ManagerSelection) {
  if (left.kind !== right.kind) return false;
  return left.kind !== "environment" ||
    (right.kind === "environment" && left.environmentId === right.environmentId);
}
