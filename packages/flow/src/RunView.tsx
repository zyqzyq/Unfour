import { useEffect, useState } from "react";
import type { FlowRun, FlowStepRun } from "@unfour/command-client";
import { Button, useI18n } from "@unfour/ui";

export function RunView({ run, cancel }: { run: FlowRun; cancel: () => void }) {
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
      {run.status === "running" && <Button variant="secondary" onClick={cancel}>{t("flow.cancel")}</Button>}
    </div>
    <code className="break-all text-xs">{run.id}</code>
    {run.error && <p role="alert">{run.error}</p>}
    {run.steps.map((step) => {
      const latest = [...step.attempts].reverse().find((attempt) => attempt.output != null);
      const latestError = [...step.attempts].reverse().find((attempt) => attempt.error);
      return <details key={step.stepId} className="border-b border-[var(--u-color-border)] py-2" open={["running", "failed", "timedOut", "interrupted"].includes(step.status)}>
        <summary className="cursor-pointer">
          {run.definition.steps.find((s) => s.id === step.stepId)?.name} · {t(`flow.status.${step.status}`)} · {t("flow.elapsed")}: {elapsed(step)} ms · {step.attempts.length} {t("flow.attempts")}
        </summary>
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
