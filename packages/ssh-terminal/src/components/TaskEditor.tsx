import { useTaskStepDrag } from "../hooks/useTaskStepDrag";
import type {
  SshConnection,
  SshTaskSaveInput,
  SshTaskStepConfig,
  SshTaskStepInput,
  SshTaskStepType,
} from "@unfour/command-client";
import { useEffect, useState } from "react";
import { Badge, Button, Input, Select, useI18n } from "@unfour/ui";
import { Play, Save } from "lucide-react";
import {
  createTaskStep,
  detectTaskInputs,
  duplicateTaskStep,
  moveTaskStep,
  removeTaskStep,
} from "../model/task-template";
import { AddStepMenu, StepInsertSlot, StepRow } from "./TaskEditorSteps";

export function TaskEditor({
  connections,
  draft,
  onChange,
  onRun,
  onSave,
  runDisabledReason,
  saving,
}: {
  connections: SshConnection[];
  draft: SshTaskSaveInput;
  onChange: (draft: SshTaskSaveInput) => void;
  onRun: () => void;
  onSave: () => void;
  runDisabledReason?: string | null;
  saving: boolean;
}) {
  const { t } = useI18n();
  const inputs = detectTaskInputs(draft.steps);
  const [expandedIndex, setExpandedIndex] = useState<number | null>(
    draft.steps.length ? 0 : null,
  );
  const [showDescription, setShowDescription] = useState(Boolean(draft.description.trim()));
  const [advancedOpen, setAdvancedOpen] = useState(false);
  const { dragIndex, overIndex, onStepDragHandlePointerDown } = useTaskStepDrag(draft, onChange, setExpandedIndex);
  const canRun = !runDisabledReason && !saving;

  useEffect(() => {
    if (expandedIndex === null) return;
    if (expandedIndex >= draft.steps.length) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- clamp expansion when steps shrink below the active index
      setExpandedIndex(draft.steps.length ? draft.steps.length - 1 : null);
    }
  }, [draft.steps.length, expandedIndex]);

  function updateStep(index: number, patch: Partial<SshTaskStepInput>) {
    onChange({
      ...draft,
      steps: draft.steps.map((step, itemIndex) =>
        itemIndex === index ? { ...step, ...patch } : step,
      ),
    });
  }

  function updateConfig(index: number, key: string, value: string | number | boolean) {
    const step = draft.steps[index];
    updateStep(index, {
      configJson: {
        ...(step.configJson as unknown as Record<string, unknown>),
        [key]: value,
      } as SshTaskStepConfig,
    });
  }

  function addStep(stepType: SshTaskStepType, atIndex?: number) {
    const step = createTaskStep(stepType, draft.steps.length);
    const nextSteps =
      atIndex === undefined
        ? [...draft.steps, step]
        : [
            ...draft.steps.slice(0, atIndex),
            step,
            ...draft.steps.slice(atIndex),
          ].map((item, position) => ({ ...item, position }));
    onChange({ ...draft, steps: nextSteps });
    setExpandedIndex(atIndex ?? nextSteps.length - 1);
    setAdvancedOpen(false);
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-[var(--u-size-section-toolbar)] shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] px-2">
        <Input
          aria-label={t("ssh.tasks.editor.name")}
          className="h-7 min-w-0 max-w-[420px] flex-[1_1_320px]"
          id="ssh-task-name"
          maxLength={128}
          onChange={(event) => onChange({ ...draft, name: event.target.value })}
          placeholder={t("ssh.tasks.editor.untitled")}
          title={draft.name || t("ssh.tasks.editor.untitled")}
          value={draft.name}
        />
        <Select
          aria-label={t("ssh.tasks.editor.defaultConnection")}
          className="h-7 max-w-[260px] shrink-0"
          id="ssh-task-default-connection"
          onChange={(event) =>
            onChange({
              ...draft,
              defaultConnectionId: event.target.value || null,
            })
          }
          value={draft.defaultConnectionId ?? ""}
        >
          <option value="">{t("ssh.tasks.editor.noDefaultConnection")}</option>
          {connections.map((connection) => (
            <option key={connection.id} value={connection.id}>
              {connection.name} · {connection.host}
            </option>
          ))}
        </Select>
        <Button
          onClick={() => setShowDescription((open) => !open)}
          size="sm"
          variant="ghost"
        >
          {showDescription
            ? t("ssh.tasks.editor.hideDescription")
            : t("ssh.tasks.editor.showDescription")}
        </Button>
        {inputs.length > 0 && (
          <div className="flex min-w-0 flex-1 items-center gap-1 overflow-x-auto">
            <span className="shrink-0 text-[11px] text-[var(--u-color-text-soft)]">
              {t("ssh.tasks.editor.detectedInputs")}
            </span>
            {inputs.map((input) => (
              <Badge key={input}>{`{{${input}}}`}</Badge>
            ))}
          </div>
        )}
        <div className="ml-auto flex shrink-0 items-center gap-1">
          <Button
            disabled={!draft.name.trim() || saving}
            onClick={onSave}
            size="sm"
            title="Ctrl+S"
            variant="secondary"
          >
            <Save size={13} />
            {saving ? t("ssh.tasks.actions.saving") : t("ssh.tasks.actions.save")}
          </Button>
          <Button
            disabled={!canRun}
            onClick={onRun}
            size="sm"
            title={runDisabledReason ?? "Ctrl+Enter"}
          >
            <Play size={13} />
            {saving ? t("ssh.tasks.actions.saving") : t("ssh.tasks.actions.run")}
          </Button>
        </div>
      </div>

      {showDescription && (
        <div className="shrink-0 border-b border-[var(--u-color-border)] px-2 py-2">
          <textarea
            aria-label={t("ssh.tasks.editor.description")}
            className="min-h-14 w-full resize-y rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] px-2 py-1.5 text-[13px] text-[var(--u-color-text)] outline-none transition-colors focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)]"
            id="ssh-task-description"
            maxLength={2000}
            onChange={(event) => onChange({ ...draft, description: event.target.value })}
            placeholder={t("ssh.tasks.editor.descriptionPlaceholder")}
            value={draft.description}
          />
        </div>
      )}

      <div className="min-h-0 flex-1 overflow-y-auto p-2">
        <div className="mx-auto flex max-w-[920px] flex-col">
          <div className="mb-1.5 flex items-center justify-between gap-2">
            <div className="min-w-0">
              <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
                {t("ssh.tasks.editor.steps")}
              </h3>
              <p className="text-[11px] text-[var(--u-color-text-soft)]">
                {t("ssh.tasks.editor.inputHint")}
              </p>
            </div>
            <AddStepMenu onAdd={(type) => addStep(type)} />
          </div>

          {draft.steps.length ? (
            <div className="flex flex-col">
              {draft.steps.map((step, index) => (
                <div key={step.id ?? `${step.stepType}-${index}`}>
                  {index > 0 && (
                    <StepInsertSlot onAdd={(type) => addStep(type, index)} />
                  )}
                  <StepRow
                    advancedOpen={advancedOpen && expandedIndex === index}
                    dragOver={overIndex === index && dragIndex !== index}
                    dragging={dragIndex === index}
                    expanded={expandedIndex === index}
                    index={index}
                    onConfigChange={(key, value) => updateConfig(index, key, value)}
                    onDragHandlePointerDown={(event) =>
                      onStepDragHandlePointerDown(index, event)
                    }
                    onDuplicate={() => {
                      onChange({
                        ...draft,
                        steps: duplicateTaskStep(draft.steps, index),
                      });
                      setExpandedIndex(index + 1);
                    }}
                    onMove={(direction) => {
                      onChange({
                        ...draft,
                        steps: moveTaskStep(draft.steps, index, direction),
                      });
                      setExpandedIndex(index + direction);
                    }}
                    onRemove={() => {
                      onChange({
                        ...draft,
                        steps: removeTaskStep(draft.steps, index),
                      });
                      setExpandedIndex((current) => {
                        if (current === null) return null;
                        if (draft.steps.length <= 1) return null;
                        if (current > index) return current - 1;
                        if (current === index) return Math.max(0, index - 1);
                        return current;
                      });
                    }}
                    onToggleAdvanced={() => {
                      setExpandedIndex(index);
                      setAdvancedOpen((open) => (expandedIndex === index ? !open : true));
                    }}
                    onToggleExpand={() => {
                      setExpandedIndex((current) => (current === index ? null : index));
                      setAdvancedOpen(false);
                    }}
                    onUpdate={(patch) => updateStep(index, patch)}
                    step={step}
                    stepCount={draft.steps.length}
                  />
                </div>
              ))}
              <div className="mt-1 flex justify-center py-1">
                <AddStepMenu onAdd={(type) => addStep(type)} variant="ghost" />
              </div>
            </div>
          ) : (
            <div className="flex flex-col items-center gap-3 border border-dashed border-[var(--u-color-border)] px-4 py-8 text-center">
              <p className="text-[12px] text-[var(--u-color-text-muted)]">
                {t("ssh.tasks.editor.noSteps")}
              </p>
              <p className="text-[11px] text-[var(--u-color-text-soft)]">
                {t("ssh.tasks.editor.inputHint")}
              </p>
              <AddStepMenu onAdd={(type) => addStep(type)} />
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
