import type {
  SshConnection,
  SshTaskSaveInput,
  SshTaskStepConfig,
  SshTaskStepInput,
  SshTaskStepType,
} from "@unfour/command-client";
import type { ReactNode } from "react";
import { Badge, Button, IconButton, Input, Select, useI18n } from "@unfour/ui";
import { ArrowDown, ArrowUp, Copy, Play, Save, Trash2 } from "lucide-react";
import {
  createTaskStep,
  detectTaskInputs,
  duplicateTaskStep,
  moveTaskStep,
  removeTaskStep,
} from "../model/task-template";

export function TaskEditor({
  connections,
  draft,
  onChange,
  onRun,
  onSave,
  saving,
}: {
  connections: SshConnection[];
  draft: SshTaskSaveInput;
  onChange: (draft: SshTaskSaveInput) => void;
  onRun: () => void;
  onSave: () => void;
  saving: boolean;
}) {
  const { t } = useI18n();
  const inputs = detectTaskInputs(draft.steps);

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

  function addStep(stepType: SshTaskStepType) {
    onChange({
      ...draft,
      steps: [...draft.steps, createTaskStep(stepType, draft.steps.length)],
    });
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-[var(--u-size-section-toolbar)] shrink-0 items-center justify-between border-b border-[var(--u-color-border)] px-2">
        <span className="truncate text-[13px] font-semibold text-[var(--u-color-text)]">
          {draft.name || t("ssh.tasks.editor.untitled")}
        </span>
        <div className="flex items-center gap-1">
          <Button disabled={!draft.name.trim() || saving} onClick={onSave} size="sm" variant="secondary">
            <Save size={13} />
            {saving ? t("ssh.tasks.actions.saving") : t("ssh.tasks.actions.save")}
          </Button>
          <Button disabled={!draft.id || draft.steps.every((step) => !step.enabled)} onClick={onRun} size="sm">
            <Play size={13} />
            {t("ssh.tasks.actions.run")}
          </Button>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto p-3">
        <div className="mx-auto flex max-w-[900px] flex-col gap-4">
          <section className="grid grid-cols-[minmax(0,1fr)_260px] gap-3">
            <Field label={t("ssh.tasks.editor.name")}>
              <Input
                id="ssh-task-name"
                maxLength={128}
                onChange={(event) => onChange({ ...draft, name: event.target.value })}
                value={draft.name}
              />
            </Field>
            <Field label={t("ssh.tasks.editor.defaultConnection")}>
              <Select
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
            </Field>
            <Field className="col-span-2" label={t("ssh.tasks.editor.description")}>
              <textarea
                className="min-h-16 w-full resize-y rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-surface)] px-2 py-1.5 text-[13px] text-[var(--u-color-text)] outline-none transition-colors focus:border-[var(--u-color-focus)] focus:ring-2 focus:ring-[color:color-mix(in_srgb,var(--u-color-focus)_16%,transparent)]"
                id="ssh-task-description"
                maxLength={2000}
                onChange={(event) => onChange({ ...draft, description: event.target.value })}
                value={draft.description}
              />
            </Field>
          </section>

          <section>
            <div className="mb-2 flex items-center justify-between">
              <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
                {t("ssh.tasks.editor.steps")}
              </h3>
              <div className="flex gap-1">
                <Button onClick={() => addStep("command")} size="sm" variant="outline">
                  {t("ssh.tasks.actions.addCommand")}
                </Button>
                <Button onClick={() => addStep("upload")} size="sm" variant="outline">
                  {t("ssh.tasks.actions.addUpload")}
                </Button>
                <Button onClick={() => addStep("download")} size="sm" variant="outline">
                  {t("ssh.tasks.actions.addDownload")}
                </Button>
              </div>
            </div>
            <div className="flex flex-col gap-2">
              {draft.steps.length ? (
                draft.steps.map((step, index) => (
                  <StepEditor
                    index={index}
                    key={step.id ?? `${step.stepType}-${index}`}
                    onConfigChange={(key, value) => updateConfig(index, key, value)}
                    onDuplicate={() => onChange({ ...draft, steps: duplicateTaskStep(draft.steps, index) })}
                    onMove={(direction) => onChange({ ...draft, steps: moveTaskStep(draft.steps, index, direction) })}
                    onRemove={() => onChange({ ...draft, steps: removeTaskStep(draft.steps, index) })}
                    onUpdate={(patch) => updateStep(index, patch)}
                    step={step}
                    stepCount={draft.steps.length}
                  />
                ))
              ) : (
                <div className="border border-dashed border-[var(--u-color-border)] p-4 text-center text-[12px] text-[var(--u-color-text-muted)]">
                  {t("ssh.tasks.editor.noSteps")}
                </div>
              )}
            </div>
          </section>

          <section className="border-t border-[var(--u-color-border)] pt-3">
            <h3 className="text-[12px] font-semibold text-[var(--u-color-text)]">
              {t("ssh.tasks.editor.detectedInputs")}
            </h3>
            <div className="mt-2 flex flex-wrap gap-1.5">
              {inputs.length ? (
                inputs.map((input) => <Badge key={input}>{input}</Badge>)
              ) : (
                <span className="text-[12px] text-[var(--u-color-text-muted)]">
                  {t("ssh.tasks.editor.noDetectedInputs")}
                </span>
              )}
            </div>
          </section>
        </div>
      </div>
    </div>
  );
}

function StepEditor({
  index,
  onConfigChange,
  onDuplicate,
  onMove,
  onRemove,
  onUpdate,
  step,
  stepCount,
}: {
  index: number;
  onConfigChange: (key: string, value: string | number | boolean) => void;
  onDuplicate: () => void;
  onMove: (direction: -1 | 1) => void;
  onRemove: () => void;
  onUpdate: (patch: Partial<SshTaskStepInput>) => void;
  step: SshTaskStepInput;
  stepCount: number;
}) {
  const { t } = useI18n();
  const config = step.configJson as unknown as Record<string, string | number | boolean>;
  return (
    <article className={`border border-[var(--u-color-border)] bg-[var(--u-color-surface)] ${step.enabled ? "" : "opacity-60"}`}>
      <div className="flex h-9 items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface-subtle)] px-2">
        <span className="w-5 text-center font-mono text-[11px] text-[var(--u-color-text-soft)]">
          {index + 1}
        </span>
        <Badge>{t(`ssh.tasks.stepTypes.${step.stepType}`)}</Badge>
        <Input
          aria-label={t("ssh.tasks.editor.stepName")}
          className="h-7 min-w-0 flex-1"
          maxLength={128}
          onChange={(event) => onUpdate({ name: event.target.value })}
          value={step.name}
        />
        <label className="flex cursor-pointer items-center gap-1 text-[11px] text-[var(--u-color-text-muted)]">
          <input
            checked={step.enabled}
            onChange={(event) => onUpdate({ enabled: event.target.checked })}
            type="checkbox"
          />
          {t("ssh.tasks.editor.enabled")}
        </label>
        <IconButton disabled={index === 0} label={t("ssh.tasks.actions.moveUp")} onClick={() => onMove(-1)} size="compact">
          <ArrowUp size={12} />
        </IconButton>
        <IconButton disabled={index === stepCount - 1} label={t("ssh.tasks.actions.moveDown")} onClick={() => onMove(1)} size="compact">
          <ArrowDown size={12} />
        </IconButton>
        <IconButton label={t("ssh.tasks.actions.duplicateStep")} onClick={onDuplicate} size="compact">
          <Copy size={12} />
        </IconButton>
        <IconButton className="text-[var(--u-color-danger)]" label={t("ssh.tasks.actions.deleteStep")} onClick={onRemove} size="compact">
          <Trash2 size={12} />
        </IconButton>
      </div>
      <div className="grid grid-cols-2 gap-3 p-3">
        {step.stepType === "command" ? (
          <>
            <Field className="col-span-2" label={t("ssh.tasks.editor.command")}>
              <textarea
                className="min-h-20 w-full resize-y rounded-[var(--u-radius-sm)] border border-[var(--u-color-input)] bg-[var(--u-color-bg)] px-2 py-1.5 font-mono text-[12px] text-[var(--u-color-text)] outline-none focus:border-[var(--u-color-focus)]"
                onChange={(event) => onConfigChange("command", event.target.value)}
                value={String(config.command ?? "")}
              />
            </Field>
            <Field label={t("ssh.tasks.editor.workingDirectory")}>
              <Input onChange={(event) => onConfigChange("workingDirectory", event.target.value)} value={String(config.workingDirectory ?? "")} />
            </Field>
            <Field label={t("ssh.tasks.editor.timeoutSeconds")}>
              <Input max={3600} min={1} onChange={(event) => onConfigChange("timeoutSeconds", Number(event.target.value))} type="number" value={Number(config.timeoutSeconds ?? 300)} />
            </Field>
            <label className="col-span-2 flex cursor-pointer items-center gap-2 text-[12px] text-[var(--u-color-text-muted)]">
              <input checked={Boolean(config.continueOnError)} onChange={(event) => onConfigChange("continueOnError", event.target.checked)} type="checkbox" />
              {t("ssh.tasks.editor.continueOnError")}
            </label>
          </>
        ) : (
          <>
            <Field label={t("ssh.tasks.editor.localPath")}>
              <Input onChange={(event) => onConfigChange("localPath", event.target.value)} value={String(config.localPath ?? "")} />
            </Field>
            <Field label={t("ssh.tasks.editor.remotePath")}>
              <Input onChange={(event) => onConfigChange("remotePath", event.target.value)} value={String(config.remotePath ?? "")} />
            </Field>
            <label className="col-span-2 flex cursor-pointer items-center gap-2 text-[12px] text-[var(--u-color-text-muted)]">
              <input checked={Boolean(config.overwrite)} onChange={(event) => onConfigChange("overwrite", event.target.checked)} type="checkbox" />
              {t("ssh.tasks.editor.overwrite")}
            </label>
          </>
        )}
      </div>
    </article>
  );
}

function Field({
  children,
  className = "",
  label,
}: {
  children: ReactNode;
  className?: string;
  label: string;
}) {
  return (
    <label className={`flex min-w-0 flex-col gap-1 ${className}`}>
      <span className="text-[11px] font-medium text-[var(--u-color-text-muted)]">{label}</span>
      {children}
    </label>
  );
}
