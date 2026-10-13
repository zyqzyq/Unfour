import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  readSshTaskRunLog,
  registerSshTaskRunChannel,
  type SshTaskRunEvent,
} from "@unfour/command-client";
import type { useFeedbackErrorHandler } from "@unfour/ui";
import {
  appendTaskRunEventCache,
  cacheTaskRunLog,
  removeTaskRunEventsForTask,
} from "../model/task-run-events";

/** Own live output delivery and bounded history caches for the Tasks surface. */
export function useSshTaskRunOutput({
  active,
  activeRunId,
  handleError,
  workspaceId,
}: {
  active: boolean;
  activeRunId: string | null;
  handleError: ReturnType<typeof useFeedbackErrorHandler>;
  workspaceId: string;
}) {
  const queryClient = useQueryClient();
  const [eventsByRun, setEventsByRun] = useState<Record<string, SshTaskRunEvent[]>>({});
  const [historyLogByRun, setHistoryLogByRun] = useState<Record<string, string>>({});
  const [historyLogLoading, setHistoryLogLoading] = useState(false);
  const eventsByRunRef = useRef(eventsByRun);
  const historyLogByRunRef = useRef(historyLogByRun);
  const tasksSurfaceActiveRef = useRef(active);
  const activeRunIdRef = useRef(activeRunId);

  useEffect(() => {
    eventsByRunRef.current = eventsByRun;
    historyLogByRunRef.current = historyLogByRun;
    tasksSurfaceActiveRef.current = active;
    activeRunIdRef.current = activeRunId;
  }, [active, activeRunId, eventsByRun, historyLogByRun]);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    // Coalesce per-line task output into one React update ~per frame. Without
    // this, a verbose command (or transfer progress) re-renders thousands of
    // transcript spans and can freeze the Tasks surface.
    let pending: SshTaskRunEvent[] = [];
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flushPending = () => {
      flushTimer = null;
      if (disposed) {
        pending = [];
        return;
      }
      if (!pending.length) return;
      // Keep buffering while Connections is shown so a hidden Tasks tree does
      // not re-render on every remote line; flush when Tasks becomes active.
      if (!tasksSurfaceActiveRef.current) {
        flushTimer = setTimeout(flushPending, 250);
        return;
      }
      const batch = pending;
      pending = [];
      setEventsByRun((current) =>
        appendTaskRunEventCache(current, batch, activeRunIdRef.current),
      );
      for (const event of batch) {
        if (event.kind === "run" && event.status && event.status !== "running") {
          queryClient.invalidateQueries({
            queryKey: ["ssh-task-runs", workspaceId, event.taskId],
          });
        }
      }
    };
    registerSshTaskRunChannel((event) => {
      if (disposed) return;
      pending.push(event);
      if (pending.length > 10_000) {
        pending = pending.slice(-10_000);
      }
      if (flushTimer === null) {
        flushTimer = setTimeout(flushPending, 16);
      }
    }).then((cleanup) => {
      if (disposed) cleanup();
      else dispose = cleanup;
    });
    return () => {
      disposed = true;
      if (flushTimer !== null) clearTimeout(flushTimer);
      flushTimer = null;
      pending = [];
      dispose?.();
    };
  }, [queryClient, workspaceId]);

  const loadHistoryLog = useCallback(
    async (runId: string) => {
      const hasLiveEvents = (eventsByRunRef.current[runId]?.length ?? 0) > 0;
      if (hasLiveEvents || historyLogByRunRef.current[runId] !== undefined) {
        return;
      }

      setHistoryLogLoading(true);
      try {
        const logText = await readSshTaskRunLog(workspaceId, runId);
        setHistoryLogByRun((current) =>
          current[runId] === undefined
            ? cacheTaskRunLog(current, runId, logText)
            : current,
        );
      } catch (error) {
        handleError(error, { key: "feedback.ssh.taskLogLoadFailed" });
        setHistoryLogByRun((current) =>
          current[runId] === undefined
            ? cacheTaskRunLog(current, runId, "")
            : current,
        );
      } finally {
        setHistoryLogLoading(false);
      }
    },
    [handleError, workspaceId],
  );

  const clearTaskOutput = useCallback((taskId: string) => {
    setEventsByRun((current) => removeTaskRunEventsForTask(current, taskId));
    setHistoryLogByRun({});
  }, []);

  return { eventsByRun, historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskOutput };
}
