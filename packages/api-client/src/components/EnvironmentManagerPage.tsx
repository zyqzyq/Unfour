import { useEffect, useMemo, useState } from "react";
import { MoreHorizontal, Trash2 } from "lucide-react";
import type { ApiEnvironment, KeyValue } from "@unfour/command-client";
import {
  Button,
  ConfirmDialog,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
  IconButton,
  Input,
  useI18n,
} from "@unfour/ui";
import { useApiEnvironments } from "../hooks/useApiEnvironments";
import { formatError } from "../model/api-request-state";
import {
  findDuplicateEnvironmentName,
  nextEnvironmentName,
} from "../request-utils";
import { EnvironmentHints, KeyValueEditor } from "./KeyValueEditor";

export type EnvironmentManagerInitialMode =
  | { kind: "manage"; nonce: number }
  | { kind: "new"; nonce: number }
  | { environmentId: string; kind: "edit"; nonce: number };

type ExistingEnvironmentDraft = {
  id: string;
  kind: "existing";
  name: string;
  sourceName: string;
  sourceUpdatedAt: string;
  sourceVariables: KeyValue[];
  variables: KeyValue[];
};

type EnvironmentDraft =
  | { kind: "none" }
  | { kind: "new"; name: string; variables: KeyValue[] }
  | ExistingEnvironmentDraft;

export function EnvironmentManagerPage({
  initialMode,
  onDirtyChange,
  onSelectionChange,
  workspaceId,
}: {
  initialMode: EnvironmentManagerInitialMode;
  onDirtyChange?: (dirty: boolean) => void;
  onSelectionChange?: (environmentId: string | null) => void;
  workspaceId: string;
}) {
  const { t } = useI18n();
  const {
    createMut,
    deleteMut,
    environments,
    isLoading,
    updateMut,
  } = useApiEnvironments(workspaceId);
  const [draft, setDraft] = useState<EnvironmentDraft>({ kind: "none" });
  const [deleteTarget, setDeleteTarget] = useState<ApiEnvironment | null>(null);
  const [saveError, setSaveError] = useState<string | null>(null);

  const dirty = isDraftDirty(draft);
  const selectedId = draft.kind === "existing" ? draft.id : null;
  const selectedEnvironment = selectedId
    ? environments.find((environment) => environment.id === selectedId) ?? null
    : null;
  const duplicateName =
    draft.kind === "none"
      ? null
      : findDuplicateEnvironmentName(
          environments,
          draft.name,
          draft.kind === "existing" ? draft.id : undefined,
        );
  const saving = createMut.isPending || updateMut.isPending;

  useEffect(() => {
    onDirtyChange?.(dirty);
  }, [dirty, onDirtyChange]);

  useEffect(() => {
    onSelectionChange?.(selectedId);
  }, [onSelectionChange, selectedId]);

  useEffect(() => {
    setSaveError(null);
    if (initialMode.kind === "new") {
      setDraft({
        kind: "new",
        name: nextEnvironmentName(t("api.environment.defaultName"), environments),
        variables: [],
      });
      return;
    }
    if (initialMode.kind === "edit") {
      const target = environments.find(
        (environment) => environment.id === initialMode.environmentId,
      );
      setDraft(target ? draftFromEnvironment(target) : { kind: "none" });
      return;
    }
    const target =
      environments.find((environment) => environment.isActive) ?? environments[0] ?? null;
    setDraft(target ? draftFromEnvironment(target) : { kind: "none" });
  }, [initialMode.nonce]);

  useEffect(() => {
    if (draft.kind !== "none" || initialMode.kind === "new") {
      return;
    }
    if (initialMode.kind === "edit") {
      const target = environments.find(
        (environment) => environment.id === initialMode.environmentId,
      );
      if (target) {
        setDraft(draftFromEnvironment(target));
      }
      return;
    }
    const target =
      environments.find((environment) => environment.isActive) ?? environments[0] ?? null;
    if (target) {
      setDraft(draftFromEnvironment(target));
    }
  }, [draft.kind, environments, initialMode]);

  useEffect(() => {
    if (draft.kind !== "existing" || dirty) {
      return;
    }
    if (!selectedEnvironment) {
      const next = environments[0] ?? null;
      setDraft(next ? draftFromEnvironment(next) : { kind: "none" });
      return;
    }
    if (selectedEnvironment.updatedAt !== draft.sourceUpdatedAt) {
      setDraft(draftFromEnvironment(selectedEnvironment));
    }
  }, [dirty, draft, environments, selectedEnvironment]);

  const saveDisabled =
    draft.kind === "none" ||
    saving ||
    !draft.name.trim() ||
    Boolean(duplicateName) ||
    !dirty;

  async function saveDraft() {
    if (draft.kind === "none") {
      return;
    }
    const name = draft.name.trim();
    const variables = persistableVariables(draft.variables);
    setSaveError(null);
    try {
      if (draft.kind === "new") {
        const created = await createMut.mutateAsync(name);
        if (!variables.length) {
          setDraft(draftFromEnvironment(created));
          return;
        }
        try {
          const updated = await updateMut.mutateAsync({
            id: created.id,
            name,
            variables,
          });
          setDraft(draftFromEnvironment(updated));
        } catch (error) {
          setDraft({
            ...draftFromEnvironment(created),
            name,
            variables,
          });
          setSaveError(formatError(error));
        }
        return;
      }
      const updated = await updateMut.mutateAsync({ id: draft.id, name, variables });
      setDraft(draftFromEnvironment(updated));
    } catch (error) {
      setSaveError(formatError(error));
    }
  }

  function requestDelete(environment: ApiEnvironment) {
    setDeleteTarget(environment);
  }

  function confirmDelete() {
    if (!deleteTarget) {
      return;
    }
    deleteMut.mutate(deleteTarget.id, {
      onSuccess: (nextEnvironments) => {
        setDeleteTarget(null);
        const next = nextEnvironments[0] ?? null;
        setDraft(next ? draftFromEnvironment(next) : { kind: "none" });
      },
    });
  }

  const variableCount = useMemo(
    () => (draft.kind === "none" ? 0 : persistableVariables(draft.variables).length),
    [draft],
  );

  return (
    <div className="flex h-full min-h-0 bg-[var(--u-color-bg)]">
      <main className="min-w-0 flex-1 overflow-y-auto">
        {draft.kind === "none" ? (
          <div className="flex h-full items-center justify-center p-6 text-[12px] text-[var(--u-color-text-muted)]">
            {isLoading
              ? t("common.state.loading")
              : environments.length > 0
                ? t("api.environment.selectHint")
                : t("api.environment.noneConfigured")}
          </div>
        ) : (
          <div className="mx-auto flex h-full max-w-4xl flex-col gap-3 p-4">
            <div className="flex shrink-0 items-start justify-between gap-3 border-b border-[var(--u-color-border)] pb-3">
              <div className="min-w-0">
                <div className="flex items-center gap-2">
                  <h2 className="truncate text-[14px] font-semibold text-[var(--u-color-text)]">
                    {draft.kind === "new"
                      ? t("api.environment.newEnvironment")
                      : draft.sourceName}
                  </h2>
                  {selectedEnvironment?.isActive && (
                    <span className="inline-flex h-5 items-center gap-1.5 rounded-[var(--u-radius-sm)] px-1.5 text-[11px] font-medium text-[var(--u-color-primary)]">
                      <span className="h-1.5 w-1.5 rounded-full bg-current" />
                      {t("api.environment.activeBadge")}
                    </span>
                  )}
                  {dirty && (
                    <span className="text-[11px] text-[var(--u-color-text-soft)]">
                      {t("api.environment.unsaved")}
                    </span>
                  )}
                </div>
                <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
                  {t("api.environment.workspaceVariables", { count: variableCount })}
                </p>
              </div>
              <div className="flex shrink-0 items-center gap-2">
                {draft.kind === "existing" && selectedEnvironment && (
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <IconButton
                        disabled={deleteMut.isPending}
                        label={t("api.environment.actions")}
                        size="compact"
                        tooltip={t("api.environment.actions")}
                      >
                        <MoreHorizontal size={15} />
                      </IconButton>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="start">
                      <DropdownMenuItem
                        className="text-[var(--u-color-danger)]"
                        onSelect={() => requestDelete(selectedEnvironment)}
                      >
                        <Trash2 size={13} />
                        {t("api.environment.delete")}
                      </DropdownMenuItem>
                    </DropdownMenuContent>
                  </DropdownMenu>
                )}
                <Button disabled={saveDisabled} onClick={() => void saveDraft()} size="sm" type="button">
                  {saving ? t("api.actions.saving") : t("api.environment.save")}
                </Button>
              </div>
            </div>
            <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
              {t("api.environment.nameLabel")}
              <Input
                aria-invalid={duplicateName ? true : undefined}
                onChange={(event) =>
                  setDraft((current) =>
                    current.kind === "none"
                      ? current
                      : { ...current, name: event.target.value },
                  )
                }
                value={draft.name}
              />
            </label>
            {duplicateName && (
              <div className="text-[12px] text-[var(--u-color-danger)]">
                {t("api.environment.duplicateName", { name: draft.name.trim() })}
              </div>
            )}
            <div className="min-h-0 flex-1 overflow-y-auto">
              <KeyValueEditor
                items={draft.variables}
                onChange={(variables) =>
                  setDraft((current) =>
                    current.kind === "none" ? current : { ...current, variables },
                  )
                }
                title={t("api.environment.variablesLabel")}
              />
              <EnvironmentHints variables={draft.variables} />
            </div>
            {saveError && (
              <div className="text-[12px] text-[var(--u-color-danger)]">{saveError}</div>
            )}
          </div>
        )}
      </main>
      <ConfirmDialog
        confirmLabel={t("api.environment.delete")}
        description={
          deleteTarget
            ? t("api.environment.deleteConfirm", { name: deleteTarget.name })
            : undefined
        }
        onConfirm={confirmDelete}
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

function draftFromEnvironment(environment: ApiEnvironment): ExistingEnvironmentDraft {
  return {
    id: environment.id,
    kind: "existing",
    name: environment.name,
    sourceName: environment.name,
    sourceUpdatedAt: environment.updatedAt,
    sourceVariables: environment.variables,
    variables: environment.variables,
  };
}

function isDraftDirty(draft: EnvironmentDraft) {
  if (draft.kind === "none") {
    return false;
  }
  if (draft.kind === "new") {
    return Boolean(draft.name.trim()) || persistableVariables(draft.variables).length > 0;
  }
  return (
    draft.name !== draft.sourceName ||
    JSON.stringify(draft.variables) !== JSON.stringify(draft.sourceVariables)
  );
}

function persistableVariables(variables: KeyValue[]) {
  return variables.filter(
    (variable) => variable.key.trim() || variable.value.trim() || !variable.enabled,
  );
}
