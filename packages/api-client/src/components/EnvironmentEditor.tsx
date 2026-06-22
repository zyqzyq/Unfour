import { useState } from "react";
import { Button, Input, useI18n } from "@unfour/ui";
import type { ApiEnvironment, KeyValue } from "@unfour/command-client";
import { findDuplicateEnvironmentName } from "../request-utils";
import { EnvironmentHints, KeyValueEditor } from "./KeyValueEditor";

type EnvironmentDraft = {
  name: string;
  sourceId: string;
  sourceName: string;
  sourceUpdatedAt: string;
  sourceVariables: KeyValue[];
  variables: KeyValue[];
};

/**
 * Editor body for a single environment: name + variables + save. Shared by the
 * request-bar popover and the sidebar Environments tab so both behave the same.
 * Keeps its own draft, but follows external environment updates while clean.
 */
export function EnvironmentEditor({
  environment,
  environments = [],
  onSave,
  saveError,
  saving,
}: {
  environment: ApiEnvironment;
  environments?: ApiEnvironment[];
  onSave: (name: string, variables: KeyValue[]) => void;
  saveError?: string | null;
  saving?: boolean;
}) {
  const { t } = useI18n();
  const [draft, setDraft] = useState(() => createEnvironmentDraft(environment));

  let currentDraft = draft;
  if (environment.id !== draft.sourceId) {
    currentDraft = createEnvironmentDraft(environment);
    setDraft(currentDraft);
  } else if (environment.updatedAt !== draft.sourceUpdatedAt) {
    currentDraft = isDraftDirty(draft)
      ? {
          ...draft,
          sourceName: environment.name,
          sourceUpdatedAt: environment.updatedAt,
          sourceVariables: environment.variables,
        }
      : createEnvironmentDraft(environment);
    setDraft(currentDraft);
  }

  const dirty = isDraftDirty(currentDraft);
  const duplicateName = findDuplicateEnvironmentName(
    environments,
    currentDraft.name,
    environment.id,
  );

  return (
    <div className="space-y-2">
      <label className="grid gap-1 text-[12px] text-[var(--u-color-text-muted)]">
        {t("api.environment.nameLabel")}
        <Input
          aria-invalid={duplicateName ? true : undefined}
          onChange={(event) =>
            setDraft((current) => ({ ...current, name: event.target.value }))
          }
          value={currentDraft.name}
        />
      </label>
      {duplicateName && (
        <div className="text-[12px] text-[var(--u-color-danger)]">
          {t("api.environment.duplicateName", {
            name: currentDraft.name.trim(),
          })}
        </div>
      )}
      <KeyValueEditor
        items={currentDraft.variables}
        onChange={(variables) =>
          setDraft((current) => ({ ...current, variables }))
        }
        title={t("api.environment.variablesLabel")}
      />
      <EnvironmentHints variables={currentDraft.variables} />
      <div className="flex items-center justify-end gap-2">
        {dirty && !saving && (
          <span className="text-[11px] text-[var(--u-color-text-soft)]">
            {t("api.environment.unsaved")}
          </span>
        )}
        <Button
          disabled={saving || !currentDraft.name.trim() || Boolean(duplicateName) || !dirty}
          onClick={() => onSave(currentDraft.name.trim(), currentDraft.variables)}
          size="sm"
          type="button"
        >
          {saving ? t("api.actions.saving") : t("api.environment.save")}
        </Button>
      </div>
      {saveError && (
        <div className="text-[12px] text-[var(--u-color-danger)]">{saveError}</div>
      )}
    </div>
  );
}

function createEnvironmentDraft(environment: ApiEnvironment): EnvironmentDraft {
  return {
    name: environment.name,
    sourceId: environment.id,
    sourceName: environment.name,
    sourceUpdatedAt: environment.updatedAt,
    sourceVariables: environment.variables,
    variables: environment.variables,
  };
}

function isDraftDirty(draft: EnvironmentDraft) {
  return (
    draft.name !== draft.sourceName ||
    JSON.stringify(draft.variables) !== JSON.stringify(draft.sourceVariables)
  );
}
