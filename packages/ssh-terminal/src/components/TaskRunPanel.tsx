import type {
  SshTaskDetail,
  SshTaskRun,
  SshTaskRunEvent,
} from "@unfour/command-client";
import { isTemporarySshTaskRun } from "@unfour/command-client";
import { useCallback, useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { Button, StatusBadge, useI18n } from "@unfour/ui";
import { CircleStop, FolderOpen, X } from "lucide-react";
import { formatTaskHistoryLog, formatTaskRunError, taskDisplayStatus } from "../model/errors";
import { buildTaskRunTranscript, stepLogTargets } from "../model/task-run-transcript";
import { historyTranscript, runStepList } from "../model/task-run-history-log";
import { taskVersion } from "../model/task-run-context";
import type { LiveTaskRunEvent } from "../model/task-run-events";

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
  onRerun,
  run,
  task,
  usingStartedDefinition = false,
}: {
  cancelling: boolean;
  events: LiveTaskRunEvent[];
  logLoading?: boolean;
  logText?: string | null;
  onCancel: () => void;
  onClose: () => void;
  onRerun?: () => void;
  run: SshTaskRun;
  task: SshTaskDetail;
  usingStartedDefinition?: boolean;
}) {
  const { t } = useI18n();
  const [height, setHeight] = useState(readStoredHeight);
  const dragRef = useRef<{ startY: number; startHeight: number } | null>(null);
  const outputRef = useRef<HTMLPreElement | null>(null);
  const outputId = useId();
  type FollowMode = "following" | "reading" | "step";
  const [outputFollow, setOutputFollow] = useState<{ runId: string; mode: FollowMode }>({ runId: run.id, mode: "following" });
  // A short log may never dispatch scroll; reset on identity change itself.
  if (outputFollow.runId !== run.id) {
    setOutputFollow({ runId: run.id, mode: "following" });
  }
  const following = outputFollow.mode === "following";

  useEffect(() => {
    localStorage.setItem(HEIGHT_KEY, String(height));
  }, [height]);

  const onResizeMove = useCallback((event: PointerEvent) => {
    const drag = dragRef.current;
    if (!drag) return;
    const next = clampHeight(drag.startHeight + (drag.startY - event.clientY));
    setHeight(next);
  }, []);

  const onResizeEnd = useCallback(function endResize() {
    dragRef.current = null;
    window.removeEventListener("pointermove", onResizeMove);
    window.removeEventListener("pointerup", endResize);
    window.removeEventListener("pointercancel", endResize);
  }, [onResizeMove]);

  useEffect(() => onResizeEnd, [onResizeEnd]);

  function startResize(event: ReactPointerEvent<HTMLDivElement>) {
    event.preventDefault();
    dragRef.current = { startY: event.clientY, startHeight: height };
    window.addEventListener("pointermove", onResizeMove);
    window.addEventListener("pointerup", onResizeEnd);
    window.addEventListener("pointercancel", onResizeEnd);
  }

  const finalRunEvent = [...events]
    .reverse()
    .find((event) => event.kind === "run" && event.status !== "running");
  const error = finalRunEvent?.error ?? run.errorMessage;
  const status = taskDisplayStatus(finalRunEvent?.status ?? run.status, error);
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

  const liveTranscript = useMemo(
    () =>
      buildTaskRunTranscript(events, {
        stepHeader: (position, name) => t("ssh.tasks.run.stepHeader", { position, name }),
        stepError: (error) => formatTaskRunError(error, t),
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

  const transcript = useMemo(() => {
    if (liveTranscript.length || logLoading || !logText) return liveTranscript;
    return historyTranscript(logText, (text) => formatTaskHistoryLog(text, t));
  }, [liveTranscript, logLoading, logText, t]);
  const steps = useMemo(() => {
    const sameTask = run.context?.taskVersion ? run.context.taskVersion === taskVersion(task) : usingStartedDefinition;
    return runStepList(sameTask ? task.steps : [], events, transcript);
  }, [events, run.context, task, transcript, usingStartedDefinition]);
  const targets = useMemo(() => stepLogTargets(transcript), [transcript]);

  function showStepLog(stepId: string) {
    const output = outputRef.current;
    const target = output?.ownerDocument.getElementById(`${outputId}-${targets.get(stepId)?.key}`);
    if (!output || !target) return;
    setOutputFollow({ runId: run.id, mode: "step" });
    output.scrollTop += target.getBoundingClientRect().top - output.getBoundingClientRect().top;
    output.focus({ preventScroll: true });
  }

  useLayoutEffect(() => {
    const node = outputRef.current;
    if (!node || !following) return;
    node.scrollTop = node.scrollHeight;
  }, [transcript, logLoading, following, run.id]);

  return (
    <section
      aria-live="polite"
      className="flex shrink-0 flex-col border-t border-[var(--u-color-border)] bg-[var(--u-color-canvas)]"
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
      <div className="flex h-[var(--u-size-section-toolbar)] items-center justify-between gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-surface)] px-4">
        <div className="flex min-w-0 items-center gap-2">
          <span className="truncate text-[12px] font-semibold text-[var(--u-color-text)]">
            {task.task.name}
          </span>
          <StatusBadge tone={statusTone(status)}>
            {t(`ssh.tasks.run.status.${status}`)}
          </StatusBadge>
          {isTemporarySshTaskRun(run) && <span className="text-[11px] text-[var(--u-color-text-muted)]" title={t("ssh.tasks.run.temporaryHelp")}>{t("ssh.tasks.run.memoryOnly")}</span>}
          {running && currentStep && (
            <span className="truncate text-[11px] text-[var(--u-color-text-muted)]">
              {t("ssh.tasks.run.currentStep", { name: currentStep.stepName ?? "" })}
            </span>
          )}
        </div>
        <div className="flex items-center gap-1">
          {!following && <Button onClick={() => {
            setOutputFollow({ runId: run.id, mode: "following" });
            outputRef.current?.focus({ preventScroll: true });
          }} size="sm" variant="secondary">{t("ssh.tasks.run.followOutput")}</Button>}
          {!running && onRerun && <Button onClick={onRerun} size="sm" variant="secondary">{t("ssh.tasks.actions.rerun")}</Button>}
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
      <div className={`grid min-h-0 flex-1 ${steps.length ? "grid-cols-[300px_minmax(0,1fr)]" : "grid-cols-1"}`}>
        {steps.length > 0 && <ol className="min-h-0 overflow-y-auto border-r border-[var(--u-color-border)] bg-[var(--u-color-surface)] p-2">
          {steps.map((step) => {
              const event = stepEvents.get(step.id);
              const transfer = transfers.get(step.id);
              const observedStatus = event?.status ?? targets.get(step.id)?.recordedStatus;
              const stepStatus = taskDisplayStatus(observedStatus ?? "pending", event?.error);
              const showStatus = running || (observedStatus && observedStatus !== "running");
              const progress = transfer?.totalBytes
                ? Math.min(
                    100,
                    Math.round(((transfer.transferredBytes ?? 0) / transfer.totalBytes) * 100),
                  )
                : null;
              return (
                <li className="mb-1 rounded-[var(--u-radius-md)] border border-[var(--u-color-border)] bg-[var(--u-color-canvas)]" key={step.id}>
                  <button
                    aria-controls={outputId}
                    className="block w-full rounded-[var(--u-radius-md)] px-2 py-2.5 text-left enabled:cursor-pointer enabled:hover:bg-[var(--u-color-surface-hover)] focus-visible:outline-2 focus-visible:outline-[var(--u-color-focus)]"
                    disabled={!targets.has(step.id)}
                    onClick={() => showStepLog(step.id)}
                    title={targets.has(step.id) ? t("ssh.tasks.run.jumpToStep", { name: step.name }) : t("ssh.tasks.run.stepLogUnavailable")}
                    type="button"
                  >
                    <span className="flex items-center gap-2 text-[12px]">
                      <span className="w-5 text-right font-mono text-[var(--u-color-text-soft)]">
                        {step.position + 1}
                      </span>
                      <span className="min-w-0 flex-1 truncate text-[var(--u-color-text)]">
                        {step.name}
                      </span>
                      {showStatus && <StatusBadge tone={statusTone(stepStatus)}>
                        {t(`ssh.tasks.run.status.${stepStatus}`)}
                      </StatusBadge>}
                    </span>
                    {event?.durationMs != null || event?.exitCode != null ? (
                      <span className="ml-7 mt-0.5 block text-[10px] text-[var(--u-color-text-soft)]">
                        {event?.durationMs !== null && event?.durationMs !== undefined
                          ? `${event.durationMs} ms`
                          : ""}
                        {event?.exitCode !== null && event?.exitCode !== undefined
                          ? ` · ${t("ssh.tasks.run.exitCode", { code: event.exitCode })}`
                          : ""}
                      </span>
                    ) : null}
                    {progress !== null && (
                      <span className="ml-7 mt-1 block h-1 overflow-hidden bg-[var(--u-color-surface-muted)]">
                        <span
                          className="block h-full bg-[var(--u-color-primary)] transition-[width] duration-150"
                          style={{ width: `${progress}%` }}
                        />
                      </span>
                    )}
                  </button>
                  {event?.error && (
                    <p className="ml-9 mr-2 pb-2 select-text text-[10px] text-[var(--u-color-danger)]">
                      {formatTaskRunError(event.error, t)}
                    </p>
                  )}
                </li>
              );
            })}
        </ol>}
        <pre
          aria-label={t("ssh.tasks.run.output")}
          id={outputId}
          className="min-h-0 overflow-auto whitespace-pre-wrap break-words bg-[var(--u-color-canvas)] p-4 font-mono text-[12px] leading-6 text-[var(--u-color-text)]"
          onScroll={(event) => {
            const node = event.currentTarget;
            // Kept-alive modules have no viewport while hidden.
            if (!node.clientHeight) return;
            const atBottom = node.scrollHeight - node.scrollTop - node.clientHeight <= 24;
            const mode = atBottom ? "following" : "reading";
            // A jump to the last step can land at the bottom. Only the Follow
            // button should restart automatic scrolling after a step jump.
            setOutputFollow((current) => current.runId === run.id && (current.mode === "step" || current.mode === mode)
              ? current : { runId: run.id, mode });
          }}
          ref={outputRef}
          tabIndex={0}
        >
          {transcript.length ? (
            transcript.map((line) => (
              <span
                className={
                  line.kind === "header"
                    ? "mt-1 block font-semibold text-[var(--u-color-text-muted)] first:mt-0"
                    : line.kind === "command"
                      ? "block font-medium text-[var(--u-color-primary)]"
                      : line.kind === "meta" || line.kind === "transfer"
                        ? "block text-[var(--u-color-text-soft)]"
                        : line.kind === "error" || line.stream === "stderr"
                          ? "text-[var(--u-color-danger)]"
                          : undefined
                }
                key={line.key}
                id={line.stepId && targets.get(line.stepId)?.key === line.key ? `${outputId}-${line.key}` : undefined}
              >
                {line.kind === "header" || line.kind === "meta" || line.kind === "transfer"
                  ? `${line.text}\n`
                  : line.text}
              </span>
            ))
          ) : logLoading ? (
            t("ssh.tasks.run.loadingLog")
          ) : error ? (
            <span className="text-[var(--u-color-danger)]">{formatTaskRunError(error, t)}</span>
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
  if (status === "outcomeUncertain") return "warning";
  if (status === "cancelled") return "warning";
  if (status === "running") return "warning";
  return "neutral";
}
