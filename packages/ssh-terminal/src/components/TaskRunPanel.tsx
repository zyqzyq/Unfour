import type {
  SshTaskDetail,
  SshTaskRun,
  SshTaskRunEvent,
} from "@unfour/command-client";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button, StatusBadge, useI18n } from "@unfour/ui";
import { CircleStop, FolderOpen, X } from "lucide-react";
import { buildTaskRunTranscript } from "../model/task-run-transcript";

const HEIGHT_KEY = "unfour.ssh.task-run-panel-height";
const MIN_HEIGHT = 160;
const MAX_HEIGHT = 560;
const DEFAULT_HEIGHT = 300;

export function TaskRunPanel({
  cancelling,
  events,
  logLoading,
  logText,
  onCancel,
  onClose,
  run,
  task,
}: {
  cancelling: boolean;
  events: SshTaskRunEvent[];
  logLoading?: boolean;
  logText?: string | null;
  onCancel: () => void;
  onClose: () => void;
  run: SshTaskRun;
  task: SshTaskDetail;
}) {
  const { t } = useI18n();
  const [height, setHeight] = useState(readStoredHeight);
  const dragRef = useRef<{ startY: number; startHeight: number } | null>(null);
  const outputRef = useRef<HTMLPreElement | null>(null);

  useEffect(() => {
    localStorage.setItem(HEIGHT_KEY, String(height));
  }, [height]);

  const onResizeMove = useCallback((event: PointerEvent) => {
    const drag = dragRef.current;
    if (!drag) return;
    const next = clampHeight(drag.startHeight + (drag.startY - event.clientY));
    setHeight(next);
  }, []);

  const onResizeEnd = useCallback(() => {
    dragRef.current = null;
    window.removeEventListener("pointermove", onResizeMove);
    window.removeEventListener("pointerup", onResizeEnd);
  }, [onResizeMove]);

  function startResize(event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    dragRef.current = { startY: event.clientY, startHeight: height };
    window.addEventListener("pointermove", onResizeMove);
    window.addEventListener("pointerup", onResizeEnd);
  }

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
  const currentStep = [...stepEvents.values()]
    .reverse()
    .find((event) => event.status === "running");

  const transcript = useMemo(
    () =>
      buildTaskRunTranscript(events, {
        stepHeader: (position, name) => t("ssh.tasks.run.stepHeader", { position, name }),
        stepDone: (stepStatus, detail) =>
          detail
            ? t("ssh.tasks.run.stepDoneWithDetail", {
                status: t(`ssh.tasks.run.status.${stepStatus}`),
                detail,
              })
            : t("ssh.tasks.run.stepDone", {
                status: t(`ssh.tasks.run.status.${stepStatus}`),
              }),
        transfer: (direction, transferred, total, speed) => {
          const directionLabel =
            direction === "upload" || direction === "download"
              ? t(`ssh.tasks.stepTypes.${direction}`)
              : direction;
          return t("ssh.tasks.run.transferProgress", {
            direction: directionLabel,
            transferred,
            total,
            speed,
          });
        },
      }),
    [events, t],
  );

  const hasLiveTranscript = transcript.length > 0;
  const historyLog = (logText ?? "").trim();
  const showHistoryLog = !hasLiveTranscript && !running && Boolean(historyLog);

  useEffect(() => {
    const node = outputRef.current;
    if (!node) return;
    node.scrollTop = node.scrollHeight;
  }, [transcript, historyLog, logLoading]);

  return (
    <section
      aria-live="polite"
      className="flex shrink-0 flex-col border-t border-[var(--u-color-border)] bg-[var(--u-color-bg)]"
      style={{ height }}
    >
      <div
        aria-label={t("ssh.tasks.run.resize")}
        className="flex h-1.5 cursor-row-resize items-center justify-center bg-[var(--u-color-surface-subtle)] hover:bg-[var(--u-color-border)]"
        onPointerDown={startResize}
        role="separator"
      >
        <span className="h-0.5 w-8 rounded-full bg-[var(--u-color-border-strong)]" />
      </div>
      <div className="flex h-[var(--u-size-section-toolbar)] items-center justify-between border-b border-[var(--u-color-border)] px-2">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[12px] font-semibold text-[var(--u-color-text)]">
            {task.task.name}
          </span>
          <StatusBadge tone={statusTone(status)}>
            {t(`ssh.tasks.run.status.${status}`)}
          </StatusBadge>
          {currentStep && (
            <span className="truncate text-[11px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.run.currentStep", { name: currentStep.stepName ?? "" })}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          {run.logPath ? (
            <Button
              onClick={() => void revealItemInDir(run.logPath).catch(() => undefined)}
              size="sm"
              variant="ghost"
            >
              <FolderOpen size={13} />
              {t("ssh.tasks.run.openLog")}
            </Button>
          ) : null}
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
          {task.steps
            .filter((step) => step.enabled)
            .map((step) => {
              const event = stepEvents.get(step.id);
              const transfer = transfers.get(step.id);
              const stepStatus = event?.status ?? "pending";
              const progress = transfer?.totalBytes
                ? Math.min(
                    100,
                    Math.round(((transfer.transferredBytes ?? 0) / transfer.totalBytes) * 100),
                  )
                : null;
              return (
                <li className="px-2 py-1.5" key={step.id}>
                  <div className="flex items-center gap-2 text-[12px]">
                    <span className="w-5 text-right font-mono text-[var(--u-color-text-soft)]">
                      {step.position + 1}
                    </span>
                    <span className="min-w-0 flex-1 truncate text-[var(--u-color-text)]">
                      {step.name}
                    </span>
                    <StatusBadge tone={statusTone(stepStatus)}>
                      {t(`ssh.tasks.run.status.${stepStatus}`)}
                    </StatusBadge>
                  </div>
                  {(event?.durationMs !== null && event?.durationMs !== undefined) ||
                  event?.exitCode !== null ? (
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
                      <div
                        className="h-full bg-[var(--u-color-primary)] transition-[width] duration-150"
                        style={{ width: `${progress}%` }}
                      />
                    </div>
                  )}
                  {event?.error && (
                    <p className="ml-7 mt-1 text-[10px] text-[var(--u-color-danger)]">
                      {event.error}
                    </p>
                  )}
                </li>
              );
            })}
        </ol>
        <pre
          className="min-h-0 overflow-auto whitespace-pre-wrap break-words p-2 font-mono text-[11px] leading-5 text-[var(--u-color-text)]"
          ref={outputRef}
        >
          {hasLiveTranscript ? (
            transcript.map((line) => (
              <span
                className={
                  line.kind === "header"
                    ? "mt-1 block font-semibold text-[var(--u-color-text-muted)] first:mt-0"
                    : line.kind === "meta" || line.kind === "transfer"
                      ? "block text-[var(--u-color-text-soft)]"
                      : line.kind === "error" || line.stream === "stderr"
                        ? "text-[var(--u-color-danger)]"
                        : undefined
                }
                key={line.key}
              >
                {line.kind === "header" || line.kind === "meta" || line.kind === "transfer"
                  ? `${line.text}\n`
                  : line.text}
              </span>
            ))
          ) : logLoading ? (
            t("ssh.tasks.run.loadingLog")
          ) : showHistoryLog ? (
            historyLog
          ) : run.errorMessage ? (
            <span className="text-[var(--u-color-danger)]">{run.errorMessage}</span>
          ) : running || stepEvents.size > 0 ? (
            t("ssh.tasks.run.waitingForOutput")
          ) : (
            t("ssh.tasks.run.historyReplayHint")
          )}
        </pre>
      </div>
    </section>
  );
}

function readStoredHeight() {
  const raw = localStorage.getItem(HEIGHT_KEY);
  const parsed = raw ? Number(raw) : DEFAULT_HEIGHT;
  return Number.isFinite(parsed) ? clampHeight(parsed) : DEFAULT_HEIGHT;
}

function clampHeight(value: number) {
  return Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, Math.round(value)));
}

function statusTone(status: string): "neutral" | "success" | "warning" | "danger" {
  if (status === "success") return "success";
  if (status === "failed") return "danger";
  if (status === "cancelled") return "warning";
  if (status === "running") return "warning";
  return "neutral";
}
