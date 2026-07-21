import type {
  SshTaskDetail,
  SshTaskRun,
  SshTaskRunEvent,
} from "@unfour/command-client";
import { Button, StatusBadge, useI18n } from "@unfour/ui";
import { CircleStop, X } from "lucide-react";

export function TaskRunPanel({
  cancelling,
  events,
  onCancel,
  onClose,
  run,
  task,
}: {
  cancelling: boolean;
  events: SshTaskRunEvent[];
  onCancel: () => void;
  onClose: () => void;
  run: SshTaskRun;
  task: SshTaskDetail;
}) {
  const { t } = useI18n();
  const finalRunEvent = [...events]
    .reverse()
    .find((event) => event.kind === "run" && event.status !== "running");
  const status = finalRunEvent?.status ?? run.status;
  const running = status === "running";
  const stepEvents = new Map<string, SshTaskRunEvent>();
  const transfers = new Map<string, SshTaskRunEvent>();
  for (const event of events) {
    if (event.kind === "step" && event.stepId) stepEvents.set(event.stepId, event);
    if (event.kind === "transfer" && event.stepId) transfers.set(event.stepId, event);
  }
  const output = events.filter((event) => event.kind === "output" && event.data);
  const currentStep = [...stepEvents.values()].reverse().find((event) => event.status === "running");

  return (
    <section className="flex h-[250px] shrink-0 flex-col border-t border-[var(--u-color-border)] bg-[var(--u-color-bg)]" aria-live="polite">
      <div className="flex h-[var(--u-size-section-toolbar)] items-center justify-between border-b border-[var(--u-color-border)] px-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[12px] font-semibold text-[var(--u-color-text)]">
            {task.task.name}
          </span>
          <StatusBadge tone={statusTone(status)}>{t(`ssh.tasks.run.status.${status}`)}</StatusBadge>
          {currentStep && (
            <span className="truncate text-[11px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.run.currentStep", { name: currentStep.stepName ?? "" })}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          {running && (
            <Button disabled={cancelling} onClick={onCancel} size="sm" variant="danger">
              <CircleStop size={13} />
              {cancelling ? t("ssh.tasks.run.cancelling") : t("ssh.tasks.actions.cancelRun")}
            </Button>
          )}
          {!running && (
            <Button onClick={onClose} size="sm" variant="ghost">
              <X size={13} />
              {t("ssh.tasks.actions.close")}
            </Button>
          )}
        </div>
      </div>
      <div className="grid min-h-0 flex-1 grid-cols-[300px_minmax(0,1fr)]">
        <ol className="min-h-0 overflow-y-auto border-r border-[var(--u-color-border)] py-1">
          {task.steps.filter((step) => step.enabled).map((step) => {
            const event = stepEvents.get(step.id);
            const transfer = transfers.get(step.id);
            const stepStatus = event?.status ?? "pending";
            const progress = transfer?.totalBytes
              ? Math.min(100, Math.round(((transfer.transferredBytes ?? 0) / transfer.totalBytes) * 100))
              : null;
            return (
              <li className="px-2 py-1.5" key={step.id}>
                <div className="flex items-center gap-2 text-[12px]">
                  <span className="w-5 text-right font-mono text-[var(--u-color-text-soft)]">{step.position + 1}</span>
                  <span className="min-w-0 flex-1 truncate text-[var(--u-color-text)]">{step.name}</span>
                  <StatusBadge tone={statusTone(stepStatus)}>{t(`ssh.tasks.run.status.${stepStatus}`)}</StatusBadge>
                </div>
                {(event?.durationMs !== null && event?.durationMs !== undefined) || event?.exitCode !== null ? (
                  <div className="ml-7 mt-0.5 text-[10px] text-[var(--u-color-text-soft)]">
                    {event?.durationMs !== null && event?.durationMs !== undefined
                      ? `${event.durationMs} ms`
                      : ""}
                    {event?.exitCode !== null && event?.exitCode !== undefined
                      ? ` · ${t("ssh.tasks.run.exitCode", { code: event.exitCode })}`
                      : ""}
                  </div>
                ) : null}
                {progress !== null && (
                  <div className="ml-7 mt-1 h-1 overflow-hidden bg-[var(--u-color-surface-muted)]">
                    <div className="h-full bg-[var(--u-color-primary)] transition-[width] duration-150" style={{ width: `${progress}%` }} />
                  </div>
                )}
                {event?.error && <p className="ml-7 mt-1 text-[10px] text-[var(--u-color-danger)]">{event.error}</p>}
              </li>
            );
          })}
        </ol>
        <pre className="min-h-0 overflow-auto whitespace-pre-wrap break-words p-2 font-mono text-[11px] leading-5 text-[var(--u-color-text)]">
          {output.length
            ? output.map((event, index) => (
                <span className={event.stream === "stderr" ? "text-[var(--u-color-danger)]" : ""} key={`${event.createdAt}-${index}`}>
                  {event.data}
                </span>
              ))
            : t("ssh.tasks.run.waitingForOutput")}
        </pre>
      </div>
    </section>
  );
}

function statusTone(status: string): "neutral" | "success" | "warning" | "danger" {
  if (status === "success") return "success";
  if (status === "failed") return "danger";
  if (status === "cancelled") return "warning";
  if (status === "running") return "warning";
  return "neutral";
}
