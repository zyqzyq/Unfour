import { emptyAction } from "./model";
import type {
  FlowAction,
  FlowCapability,
  FlowPredicate,
  FlowStep,
} from "@unfour/command-client";
import { Button, Input, Select, useI18n } from "@unfour/ui";
import { useState } from "react";

export type Resources = Record<
  FlowCapability,
  { id: string; name: string }[]
> & { connections: { id: string; name: string }[] };
export function JsonField({
  label,
  value,
  onChange,
  onValidity,
}: {
  label: string;
  value: unknown;
  onChange: (value: unknown) => void | boolean;
  onValidity?: (valid: boolean) => void;
}) {
  const { t } = useI18n();
  const [text, setText] = useState(JSON.stringify(value, null, 2));
  const [error, setError] = useState(false);
  return (
    <label className="grid gap-1 text-xs">
      {label}
      <textarea
        aria-label={label}
        aria-invalid={error}
        className="min-h-20 w-full resize-y rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2 font-mono text-xs focus:outline-[var(--u-color-focus)]"
        value={text}
        onChange={(event) => {
          setText(event.target.value);
          try {
            const parsed: unknown = JSON.parse(event.target.value);
            const valid = onChange(parsed) !== false;
            setError(!valid);
            onValidity?.(valid);
          } catch {
            setError(true);
            onValidity?.(false);
          }
        }}
      />
      {error && <span role="alert">{t("flow.invalidJson")}</span>}
    </label>
  );
}
export function StepEditor({
  step,
  after,
  resources,
  onChange,
  onRemove,
  onValidity,
}: {
  step: FlowStep;
  after: FlowStep[];
  resources: Resources;
  onChange: (step: FlowStep) => void;
  onRemove: () => void;
  onValidity: (field: string, valid: boolean) => void;
}) {
  const { t } = useI18n();
  const branches = [
    { value: "$end", label: t("flow.end") },
    ...after.map((s) => ({ value: s.id, label: s.name })),
  ];
  function editAction(
    action: FlowAction,
    update: (action: FlowAction) => void,
    probe = false,
  ) {
    return (
      <div className="grid gap-2">
        <div className="grid grid-cols-2 gap-2">
          <Select
            aria-label={t("flow.capability")}
            value={action.capability}
            onChange={(e) => {
              onValidity("arguments", true);
              update(emptyAction(e.target.value as FlowCapability));
            }}
            options={(probe
              ? ["api", "database"]
              : ["api", "ssh", "database"]
            ).map((value) => ({ value, label: t(`flow.${value}`) }))}
          />
          <Select
            aria-label={t("flow.resource")}
            value={action.resourceId}
            onChange={(e) => update({ ...action, resourceId: e.target.value })}
            options={[
              { value: "", label: t("flow.selectResource") },
              ...resources[action.capability].map((r) => ({
                value: r.id,
                label: r.name,
              })),
            ]}
          />
        </div>
        {action.capability === "ssh" && (
          <Select
            aria-label={t("flow.connection")}
            value={action.connectionId ?? ""}
            onChange={(e) =>
              update({ ...action, connectionId: e.target.value })
            }
            options={[
              { value: "", label: t("flow.selectConnection") },
              ...resources.connections.map((r) => ({
                value: r.id,
                label: r.name,
              })),
            ]}
          />
        )}
        <JsonField
          key={`${step.id}-${action.capability}`}
          label={t("flow.arguments")}
          value={action.arguments}
          onValidity={(valid) => onValidity("arguments", valid)}
          onChange={(value) => {
            if (value && typeof value === "object" && !Array.isArray(value))
              update({
                ...action,
                arguments: value as Record<string, unknown>,
              });
            else return false;
          }}
        />
      </div>
    );
  }
  return (
    <section className="grid gap-2 border-b border-[var(--u-color-border)] py-3">
      <div className="flex items-center gap-2">
        <Input
          aria-label={t("flow.stepName")}
          value={step.name}
          onChange={(e) => onChange({ ...step, name: e.target.value })}
        />
        <code className="text-xs text-[var(--u-color-text-muted)]">
          {step.id}
        </code>
        <Button size="sm" variant="ghost" onClick={onRemove}>
          {t("flow.remove")}
        </Button>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <label className="grid gap-1 text-xs">
          {t("flow.timeout")}
          <Input
            type="number"
            min={1}
            max={3600000}
            value={step.timeoutMs}
            onChange={(e) =>
              onChange({ ...step, timeoutMs: Number(e.target.value) })
            }
          />
        </label>
        {step.kind !== "condition" && (
          <label className="grid gap-1 text-xs">
            {t("flow.next")}
            <Select
              value={step.next ?? ""}
              onChange={(e) =>
                onChange({ ...step, next: e.target.value || null })
              }
              options={[{ value: "", label: t("flow.nextStep") }, ...branches]}
            />
          </label>
        )}
      </div>
      {step.kind === "action" &&
        editAction(step.action, (action) => onChange({ ...step, action }))}
      {step.kind === "poll" && (
        <>
          <p className="text-xs text-[var(--u-color-text-muted)]">
            {t("flow.probeHelp")}
          </p>
          {editAction(
            step.probe,
            (probe) => onChange({ ...step, probe }),
            true,
          )}
          <div className="grid grid-cols-2 gap-2">
            <label>
              {t("flow.interval")}
              <Input
                type="number"
                min={10}
                value={step.intervalMs}
                onChange={(e) =>
                  onChange({ ...step, intervalMs: Number(e.target.value) })
                }
              />
            </label>
            <label>
              {t("flow.maxAttempts")}
              <Input
                type="number"
                min={1}
                max={1000}
                value={step.maxAttempts}
                onChange={(e) =>
                  onChange({ ...step, maxAttempts: Number(e.target.value) })
                }
              />
            </label>
          </div>
        </>
      )}
      {(step.kind === "condition" || step.kind === "poll") && (
        <JsonField
          label={t("flow.predicate")}
          value={step.predicate}
          onValidity={(valid) => onValidity("predicate", valid)}
          onChange={(value) => {
            if (
              value &&
              typeof value === "object" &&
              "left" in value &&
              "right" in value &&
              "op" in value &&
              ["eq", "ne", "gt", "ge", "lt", "le"].includes(String(value.op))
            )
              onChange({ ...step, predicate: value as FlowPredicate });
            else return false;
          }}
        />
      )}
      {step.kind === "condition" && (
        <div className="grid grid-cols-2 gap-2">
          <label>
            {t("flow.ifTrue")}
            <Select
              value={step.ifTrue}
              options={branches}
              onChange={(e) => onChange({ ...step, ifTrue: e.target.value })}
            />
          </label>
          <label>
            {t("flow.ifFalse")}
            <Select
              value={step.ifFalse}
              options={branches}
              onChange={(e) => onChange({ ...step, ifFalse: e.target.value })}
            />
          </label>
        </div>
      )}
      {step.kind === "wait" && (
        <label>
          {t("flow.duration")}
          <Input
            type="number"
            min={0}
            value={step.durationMs}
            onChange={(e) =>
              onChange({ ...step, durationMs: Number(e.target.value) })
            }
          />
        </label>
      )}
    </section>
  );
}
