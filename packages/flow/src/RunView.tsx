import { useEffect, useState } from "react";
import type { FlowRun, FlowStepRun } from "@unfour/command-client";
import { Button, Select, useI18n } from "@unfour/ui";

export function RunView({ run, cancel, selectedStep, onSelectStep }: { run: FlowRun; cancel?: () => void; selectedStep?: string | null; onSelectStep?: (id: string) => void }) {
  const { t } = useI18n();
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (run.status !== "running") return;
    const timer = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(timer);
  }, [run.status]);
  function elapsed(step: FlowStepRun) {
    if (step.status !== "running" || !step.startedAt) return step.durationMs;
    const start = Date.parse(step.startedAt);
    return Number.isFinite(start) ? Math.max(step.durationMs, now - start, 0) : step.durationMs;
  }
  return <section className="space-y-2">
    <div className="flex items-center gap-2">
      <strong>{t(`flow.status.${run.status}`)}</strong>
      <code>r{run.definition.revision}</code>
      {run.status === "running" && cancel && <Button variant="secondary" onClick={cancel}>{t("flow.cancel")}</Button>}
    </div>
    <code className="break-all text-xs">{run.id}</code>
    {run.error && <p role="alert">{run.error}</p>}
    <p className="text-xs">{t("flow.startedAt")}: {run.startedAt}</p>
    {run.finishedAt && <p className="text-xs">{t("flow.finishedAt")}: {run.finishedAt}</p>}
    {onSelectStep && <label className="grid gap-1 text-xs">{t("flow.recordedNode")}
      <Select value={run.steps.some((step) => step.stepId === selectedStep) ? selectedStep ?? "" : ""} onChange={(event) => { if (event.target.value) onSelectStep(event.target.value); }} options={[
        { value: "", label: t("flow.selectRecordedNode") },
        ...run.steps.map((step) => ({ value: step.stepId, label: run.definition.steps.find((definition) => definition.id === step.stepId)?.name ?? step.stepId })),
      ]} />
    </label>}
    {selectedStep !== undefined && !run.steps.some((step) => step.stepId === selectedStep) && <p className="text-xs text-[var(--u-color-text-muted)]">{t("flow.selectRunNode")}</p>}
    {run.steps.filter((step) => selectedStep === undefined || step.stepId === selectedStep).map((step) => {
      const latest = [...step.attempts].reverse().find((attempt) => attempt.output != null);
      const latestError = [...step.attempts].reverse().find((attempt) => attempt.error);
      return <details key={step.stepId} className="border-b border-[var(--u-color-border)] py-2" open={selectedStep !== undefined || ["running", "failed", "timedOut", "interrupted"].includes(step.status)}>
        <summary className="cursor-pointer">
          {run.definition.steps.find((s) => s.id === step.stepId)?.name} · {t(`flow.status.${step.status}`)} · {t("flow.elapsed")}: {elapsed(step)} ms · {step.attempts.length} {t("flow.attempts")}
        </summary>
        {step.startedAt && <p className="text-xs">{t("flow.startedAt")}: {step.startedAt}</p>}
        <p className="mt-2 text-xs text-[var(--u-color-text-muted)]">{t("flow.inputValues")}</p>
        <pre className="max-h-60 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify(step.attempts[step.attempts.length - 1]?.input ?? null, null, 2)}</pre>
        {step.nextCheckAt && step.status === "running" && <p className="text-xs">{t("flow.nextCheck")}: {step.nextCheckAt}</p>}
        {step.error && <p role="alert">{step.error}</p>}
        <p className="mt-2 text-xs text-[var(--u-color-text-muted)]">{t("flow.latestResult")}</p>
        <pre className="max-h-60 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify(step.output ?? latest?.output ?? null, null, 2)}</pre>
        {latestError && <><p className="text-xs text-[var(--u-color-text-muted)]">{t("flow.latestError")} · #{latestError.number}</p><p role="alert">{latestError.error}</p></>}
        <details><summary className="text-xs">{t("flow.attemptHistory")}</summary>
          {step.attempts.map((attempt) => <pre key={attempt.number} className="mt-2 max-h-80 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify(attempt, null, 2)}</pre>)}
        </details>
      </details>;
    })}
    <details><summary>{t("flow.snapshot")}</summary><pre className="overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify({ definition: run.definition, context: run.context, resources: run.resources }, null, 2)}</pre></details>
  </section>;
}
