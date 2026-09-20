import { emptyAction, type Resources } from "./model";
import type {
  FlowAction,
  FlowCapability,
  FlowPredicate,
  FlowStep,
} from "@unfour/command-client";
import { Button, Input, Select, useI18n } from "@unfour/ui";
import { JsonField } from "./JsonField";

export type { Resources } from "./model";
export { JsonField } from "./JsonField";
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
      {(step.kind === "poll" || step.kind === "waitUntil") && <strong className="text-xs">{t("flow.waitUntil")}</strong>}
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
      {(step.kind === "poll" || step.kind === "waitUntil") && (
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
                max={60000}
                value={step.intervalMs}
                onChange={(e) =>
                  onChange({ ...step, intervalMs: Number(e.target.value) })
                }
              />
            </label>
            <details>
              <summary>{t("flow.advanced")}</summary>
            <label>
              {t("flow.maxAttempts")}
              <Input
                type="number"
                min={1}
                max={1000}
                value={step.maxAttempts ?? ""}
                placeholder={t("flow.timeoutOnly")}
                onChange={(e) =>
                  onChange(step.kind === "poll" ? { ...step, maxAttempts: Number(e.target.value) } : { ...step, maxAttempts: e.target.value ? Number(e.target.value) : null })
                }
              />
              </label>
            </details>
          </div>
        </>
      )}
      {step.kind === "waitUntil" && <>
        <JsonField label={t("flow.successWhen")} value={step.successWhen} onValidity={(valid) => onValidity("successWhen", valid)} onChange={(value) => {
          if (isPredicate(value)) onChange({ ...step, successWhen: value }); else return false;
        }} />
        <JsonField label={t("flow.failureWhen")} value={step.failureWhen ?? null} onValidity={(valid) => onValidity("failureWhen", valid)} onChange={(value) => {
          if (value === null || isPredicate(value)) onChange({ ...step, failureWhen: value }); else return false;
        }} />
        <label>{t("flow.probeErrorPolicy")}<Select value={step.probeErrorPolicy} options={[{ value: "failImmediately", label: t("flow.failImmediately") }, { value: "retryTransientErrors", label: t("flow.retryTransientErrors") }]} onChange={(e) => onChange({ ...step, probeErrorPolicy: e.target.value as "failImmediately" | "retryTransientErrors" })} /></label>
      </>}
      {(step.kind === "condition" || step.kind === "poll") && (
        <JsonField
          label={t("flow.predicate")}
          value={step.predicate}
          onValidity={(valid) => onValidity("predicate", valid)}
          onChange={(value) => {
            if (isPredicate(value))
              onChange({ ...step, predicate: value });
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

function isPredicate(value: unknown): value is FlowPredicate {
  return Boolean(value && typeof value === "object" && "left" in value && "right" in value && "op" in value && ["eq", "ne", "gt", "ge", "lt", "le", "in"].includes(String(value.op)));
}
