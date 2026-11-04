import { useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode } from "react";
import type {
  WorkspaceEnvironment,
  WorkspaceEnvironmentVariable,
  WorkspaceVariable,
  WorkspaceVariableInput,
} from "@unfour/command-client";
import { Button, Input, VariableTable, useI18n } from "@unfour/ui";
import { findDuplicateEnvironmentName, formatError, nextEnvironmentName } from "./environment-utils";

export type EnvironmentEditorMode =
  | { kind: "workspace" }
  | { environmentId: string; kind: "environment" }
  | { kind: "new" };

type ExistingEnvironmentDraft = {
  id: string;
  kind: "existing";
  name: string;
  sourceName: string;
  baseline: WorkspaceEnvironment;
  sourceVariables: WorkspaceVariableInput[];
  variables: WorkspaceVariableInput[];
};

type WorkspaceVariablesDraft = {
  kind: "workspace";
  baseline: WorkspaceVariable[];
  sourceVariables: WorkspaceVariableInput[];
  variables: WorkspaceVariableInput[];
};

type EnvironmentDraft =
  | { kind: "none" }
  | { kind: "new"; name: string; variables: WorkspaceVariableInput[] }
  | WorkspaceVariablesDraft
  | ExistingEnvironmentDraft;

export function WorkspaceEnvironmentEditor({
  environments,
  environmentsLoading,
  mode,
  modeRevision,
  onCreate,
  onDirtyChange,
  onEnvironmentCreated,
  onReplaceWorkspaceVariables,
  onUpdate,
  saving,
  workspaceVariables,
  workspaceVariablesLoading,
  variableDecoration,
}: {
  environments: WorkspaceEnvironment[];
  environmentsLoading: boolean;
  mode: EnvironmentEditorMode;
  modeRevision: number;
  onCreate: (name: string) => Promise<WorkspaceEnvironment>;
  onDirtyChange: (dirty: boolean) => void;
  onEnvironmentCreated: (environmentId: string, keepDraft?: boolean) => void;
  onReplaceWorkspaceVariables: (
    variables: WorkspaceVariableInput[],
    expected: WorkspaceVariable[],
  ) => Promise<WorkspaceVariable[]>;
  onUpdate: (input: {
    id: string;
    name: string;
    variables: WorkspaceVariableInput[];
    expected?: WorkspaceEnvironment;
  }) => Promise<WorkspaceEnvironment>;
  saving: boolean;
  workspaceVariables: WorkspaceVariable[];
  workspaceVariablesLoading: boolean;
  variableDecoration?: (
    variable: WorkspaceVariable | WorkspaceEnvironmentVariable,
  ) => ReactNode;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState<EnvironmentDraft>({ kind: "none" });
  const [saveError, setSaveError] = useState<string | null>(null);
  const [saveConflict, setSaveConflict] = useState(false);
  const latest = useRef({ draft, modeRevision, mounted: true });
  useLayoutEffect(() => {
    latest.current = { draft, modeRevision, mounted: true };
    return () => { latest.current.mounted = false; };
  }, [draft, modeRevision]);
  const dirty = isDraftDirty(draft);
  const selectedEnvironment =
    draft.kind === "existing"
      ? environments.find((environment) => environment.id === draft.id) ?? null
      : null;
  const sourceChanged = draft.kind === "workspace"
    ? JSON.stringify(workspaceVariables) !== JSON.stringify(draft.baseline)
    : draft.kind === "existing"
      ? !selectedEnvironment || environmentFingerprint(selectedEnvironment) !== environmentFingerprint(draft.baseline)
      : false;
  const conflict = dirty && sourceChanged || saveConflict;
  const duplicateName =
    draft.kind === "new" || draft.kind === "existing"
      ? findDuplicateEnvironmentName(
          environments,
          draft.name,
          draft.kind === "existing" ? draft.id : undefined,
        )
      : null;
  const persistedVariables =
    draft.kind === "none" ? [] : persistableVariables(draft.variables);
  const overridingKeys = useMemo(
    () => new Set(workspaceVariables.map((variable) => variable.key)),
    [workspaceVariables],
  );
  const decoratedVariables = isWorkspaceDraft(draft)
    ? workspaceVariables
    : selectedEnvironment?.variables ?? [];

  useEffect(() => onDirtyChange(dirty), [dirty, onDirtyChange]);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- an explicit selection resets the editor draft
    setSaveError(null);
    setSaveConflict(false);
    if (mode.kind === "new") {
      setDraft({
        kind: "new",
        name: nextEnvironmentName(t("variables.defaultEnvironmentName"), environments),
        variables: [],
      });
      return;
    }
    if (mode.kind === "workspace") {
      setDraft(draftFromWorkspaceVariables(workspaceVariables));
      return;
    }
    const target = environments.find(
      (environment) => environment.id === mode.environmentId,
    );
    setDraft(target ? draftFromEnvironment(target) : { kind: "none" });
    // eslint-disable-next-line react-hooks/exhaustive-deps -- modeRevision is the explicit selection boundary
  }, [modeRevision]);

  useEffect(() => {
    if (dirty) return;
    if (draft.kind === "none" && mode.kind === "environment") {
      const target = environments.find(
        (environment) => environment.id === mode.environmentId,
      );
      if (target) {
        // eslint-disable-next-line react-hooks/set-state-in-effect -- hydrate a selection once its query resolves
        setDraft(draftFromEnvironment(target));
      }
      return;
    }
    if (draft.kind === "workspace") {
      const next = draftFromWorkspaceVariables(workspaceVariables);
      if (sourceChanged) {
        setDraft(next);
      }
      return;
    }
    if (draft.kind === "existing") {
      if (!selectedEnvironment && !environmentsLoading) {
        setDraft({ kind: "none" });
      } else if (selectedEnvironment && sourceChanged) {
        setDraft(draftFromEnvironment(selectedEnvironment));
      }
    }
  }, [dirty, draft, environments, environmentsLoading, mode, selectedEnvironment, sourceChanged, workspaceVariables]);

  const saveDisabled =
    draft.kind === "none" ||
    saving ||
    conflict ||
    !dirty ||
    persistedVariables.some((variable) => !variable.key.trim()) ||
    hasDuplicateKeys(persistedVariables) ||
    ((draft.kind === "new" || draft.kind === "existing") &&
      (!draft.name.trim() || Boolean(duplicateName)));

  async function saveDraft() {
    if (draft.kind === "none" || saveDisabled) return;
    const variables = persistableVariables(draft.variables);
    const stillSelected = () => latest.current.mounted && latest.current.modeRevision === modeRevision;
    const acceptSaved = (saved: WorkspaceVariablesDraft | ExistingEnvironmentDraft) => {
      if (!stillSelected()) return;
      const current = latest.current.draft;
      // A later keystroke belongs to the user. Advance only its persisted
      // baseline, so our own completed save is not an external conflict.
      if (current === draft) setDraft(saved);
      else if (current.kind === "workspace" && saved.kind === "workspace") {
        setDraft({ ...saved, variables: current.variables });
      } else if ((current.kind === "existing" || current.kind === "new") && saved.kind === "existing") {
        setDraft({ ...saved, name: current.name, variables: current.variables });
      }
    };
    setSaveError(null);
    try {
      if (draft.kind === "workspace") {
        const saved = await onReplaceWorkspaceVariables(variables, draft.baseline);
        acceptSaved(draftFromWorkspaceVariables(saved));
        return;
      }
      const name = draft.name.trim();
      if (draft.kind === "new") {
        const created = await onCreate(name);
        const saved = variables.length
          ? await onUpdate({ id: created.id, name, variables, expected: created })
          : created;
        if (!stillSelected()) return;
        const keepDraft = latest.current.draft !== draft;
        acceptSaved(draftFromEnvironment(saved));
        onEnvironmentCreated(saved.id, keepDraft);
        return;
      }
      const saved = await onUpdate({ id: draft.id, name, variables, expected: draft.baseline });
      acceptSaved(draftFromEnvironment(saved));
    } catch (error) {
      if (!stillSelected()) return;
      if (typeof error === "object" && error !== null && "code" in error && (error.code === "ROW_CONFLICT" || error.code === "NOT_FOUND")) {
        setSaveConflict(true);
      } else {
        setSaveError(formatError(error));
      }
    }
  }

  if (draft.kind === "none") {
    return (
      <div className="flex h-full items-center justify-center p-6 text-[12px] text-[var(--u-color-text-muted)]">
        {environmentsLoading || workspaceVariablesLoading
          ? t("common.state.loading")
          : t("variables.selectHint")}
      </div>
    );
  }

  const isWorkspace = draft.kind === "workspace";
  const draftName = draft.kind === "new" || draft.kind === "existing" ? draft.name : "";
  return (
    <main className="h-full min-w-0 flex-1 overflow-y-auto bg-[var(--u-color-bg)]">
      <div className="mx-auto flex min-h-full max-w-5xl flex-col gap-3 p-4">
        <div className="flex shrink-0 items-start justify-between gap-3 border-b border-[var(--u-color-border)] pb-3">
          <div className="min-w-0">
            <div className="flex items-center gap-2">
              <h2 className="truncate text-[14px] font-semibold text-[var(--u-color-text)]">
                {isWorkspace
                  ? t("variables.workspaceVariables")
                  : draft.kind === "new"
                    ? t("variables.newEnvironment")
                    : draft.sourceName}
              </h2>
              {selectedEnvironment?.isActive && (
                <span className="inline-flex h-5 items-center gap-1.5 px-1.5 text-[11px] font-medium text-[var(--u-color-primary)]">
                  <span className="h-1.5 w-1.5 rounded-full bg-current" />
                  {t("variables.active")}
                </span>
              )}
              {dirty && (
                <span className="text-[11px] text-[var(--u-color-text-soft)]">
                  {t("variables.unsaved")}
                </span>
              )}
            </div>
            <p className="mt-1 text-[12px] text-[var(--u-color-text-muted)]">
              {isWorkspace
                ? t("variables.workspaceDescription")
                : t("variables.environmentDescription", {
                    count: persistedVariables.length,
                  })}
            </p>
          </div>
          <Button
            disabled={saveDisabled}
            onClick={() => void saveDraft()}
            size="sm"
            type="button"
          >
            {saving ? t("variables.saving") : t("variables.save")}
          </Button>
        </div>

        {conflict && (
          <div role="alert" className="flex items-center justify-between gap-3 text-[12px] text-[var(--u-color-danger)]">
            <span>{t("variables.externalChange")}</span>
            <Button size="sm" variant="outline" disabled={saving} onClick={() => {
              setDraft(draft.kind === "workspace" ? draftFromWorkspaceVariables(workspaceVariables) : selectedEnvironment ? draftFromEnvironment(selectedEnvironment) : { kind: "none" });
              setSaveConflict(false);
              setSaveError(null);
            }}>{t("variables.reloadLatest")}</Button>
          </div>
        )}

        {!isWorkspace && (
          <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
            {t("variables.environmentName")}
            <Input
              aria-invalid={duplicateName ? true : undefined}
              onChange={(event) =>
                setDraft((current) =>
                  current.kind === "new" || current.kind === "existing"
                    ? { ...current, name: event.target.value }
                    : current,
                )
              }
              value={draftName}
            />
          </label>
        )}
        {duplicateName && (
          <div className="text-[12px] text-[var(--u-color-danger)]">
            {t("variables.duplicateEnvironmentName", { name: draftName.trim() })}
          </div>
        )}
        <VariableTable
          items={draft.variables}
          onChange={(variables) =>
            setDraft((current) =>
              current.kind === "none" ? current : { ...current, variables },
            )
          }
          overridingKeys={isWorkspace ? undefined : overridingKeys}
          renderDecoration={
            variableDecoration
              ? (item) => {
                  const variable = decoratedVariables.find(
                    (candidate) => candidate.id === item.id,
                  );
                  return variable ? variableDecoration(variable) : null;
                }
              : undefined
          }
          title={t("variables.variablesLabel")}
        />
        {saveError && (
          <div className="text-[12px] text-[var(--u-color-danger)]">{saveError}</div>
        )}
      </div>
    </main>
  );
}

function draftFromEnvironment(environment: WorkspaceEnvironment): ExistingEnvironmentDraft {
  const variables = environment.variables.map(toInput);
  return {
    id: environment.id,
    kind: "existing",
    name: environment.name,
    sourceName: environment.name,
    baseline: environment,
    sourceVariables: variables,
    variables,
  };
}

function draftFromWorkspaceVariables(variables: WorkspaceVariable[]): WorkspaceVariablesDraft {
  const inputs = variables.map(toInput);
  return { kind: "workspace", baseline: variables, sourceVariables: inputs, variables: inputs };
}

function environmentFingerprint(environment: WorkspaceEnvironment) {
  // Activation is local selection state, not editable environment content.
  const { isActive: _isActive, ...content } = environment;
  return JSON.stringify(content);
}

function toInput(variable: WorkspaceVariable): WorkspaceVariableInput {
  return {
    id: variable.id,
    key: variable.key,
    value: variable.value,
    isSecret: variable.isSecret,
    isEnabled: variable.isEnabled,
    description: variable.description,
    sortOrder: variable.sortOrder,
  };
}

function isDraftDirty(draft: EnvironmentDraft) {
  if (draft.kind === "none") return false;
  if (draft.kind === "new") {
    return Boolean(draft.name.trim()) || persistableVariables(draft.variables).length > 0;
  }
  return (
    (draft.kind === "existing" && draft.name !== draft.sourceName) ||
    JSON.stringify(draft.variables) !== JSON.stringify(draft.sourceVariables)
  );
}

function persistableVariables(variables: WorkspaceVariableInput[]) {
  return variables
    .filter(
      (variable) =>
        variable.key.trim() ||
        variable.value ||
        variable.description?.trim() ||
        variable.isSecret ||
        !variable.isEnabled,
    )
    .map((variable, index) => ({ ...variable, sortOrder: index }));
}

function hasDuplicateKeys(variables: WorkspaceVariableInput[]) {
  const keys = new Set<string>();
  for (const variable of variables) {
    const key = variable.key.trim().toLowerCase();
    if (key && keys.has(key)) return true;
    keys.add(key);
  }
  return false;
}

function isWorkspaceDraft(draft: EnvironmentDraft): draft is WorkspaceVariablesDraft {
  return draft.kind === "workspace";
}
