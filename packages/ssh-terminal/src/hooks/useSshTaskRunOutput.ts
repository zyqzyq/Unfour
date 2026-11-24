import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { registerSshTaskRunChannel } from "@unfour/command-client";
import type { useFeedbackErrorHandler } from "@unfour/ui";
import {
  appendTaskRunEventCache,
  queueTaskRunEvent,
  removeTaskRunEventsForTask,
  type LiveTaskRunEvent,
  type PendingTaskRunEvents,
} from "../model/task-run-events";
import { useSshTaskRunHistory } from "./useSshTaskRunHistory";

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
  const [eventsByRun, setEventsByRun] = useState<Record<string, LiveTaskRunEvent[]>>({});
  const eventsByRunRef = useRef(eventsByRun);
  const pendingEventsRef = useRef<PendingTaskRunEvents>(new Map());
  const removedRunsRef = useRef(new Set<string>());
  const tasksSurfaceActiveRef = useRef(active);
  const activeRunIdRef = useRef(activeRunId);
  const { historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskHistoryLogs, clearRunHistoryLog } =
    useSshTaskRunHistory({ activeRunId, eventsByRunRef, handleError, workspaceId });

  useEffect(() => {
    tasksSurfaceActiveRef.current = active;
    activeRunIdRef.current = activeRunId;
  }, [active, activeRunId]);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    eventsByRunRef.current = {};
    removedRunsRef.current.clear();
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Live output is scoped to this workspace subscription.
    setEventsByRun({});
    // Coalesce per-line task output into one React update ~per frame. Without
    // this, a verbose command (or transfer progress) re-renders thousands of
    // transcript spans and can freeze the Tasks surface.
    const pending = pendingEventsRef.current;
    pending.clear();
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flushPending = () => {
      flushTimer = null;
      if (disposed) {
        return;
      }
      if (!pending.size) return;
      // Keep buffering while Connections is shown so a hidden Tasks tree does
      // not re-render on every remote line; flush when Tasks becomes active.
      if (!tasksSurfaceActiveRef.current) {
        flushTimer = setTimeout(flushPending, 250);
        return;
      }
      const batch = [...pending.values()];
      pending.clear();
      eventsByRunRef.current = appendTaskRunEventCache(
        eventsByRunRef.current, batch, activeRunIdRef.current,
      );
      setEventsByRun(eventsByRunRef.current);
      for (const event of batch) {
        if (event.kind === "run" && event.status && event.status !== "running") {
          queryClient.invalidateQueries({
            queryKey: ["ssh-task-runs", workspaceId, event.taskId],
          });
        }
      }
    };
    // Cache eviction and transfer coalescing may shift every array index.
    let sequence = 0;
    registerSshTaskRunChannel((event) => {
      if (disposed || removedRunsRef.current.has(event.runId)) return;
      queueTaskRunEvent(pending, { ...event, sequence: sequence++ });
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
      pending.clear();
      dispose?.();
    };
  }, [queryClient, workspaceId]);

  const clearTaskOutput = useCallback((taskId: string) => {
    if (!clearTaskHistoryLogs(taskId)) return;
    // Cleanup success is the boundary for events already received. Do not block
    // run IDs: a running task can still produce fresh output after cleanup.
    for (const [key, event] of pendingEventsRef.current) {
      if (event.taskId === taskId) pendingEventsRef.current.delete(key);
    }
    eventsByRunRef.current = removeTaskRunEventsForTask(eventsByRunRef.current, taskId);
    setEventsByRun(eventsByRunRef.current);
  }, [clearTaskHistoryLogs]);

  const clearRunOutput = useCallback((runId: string) => {
    removedRunsRef.current.add(runId);
    clearRunHistoryLog(runId);
    for (const [key, event] of pendingEventsRef.current) {
      if (event.runId === runId) pendingEventsRef.current.delete(key);
    }
    const next = { ...eventsByRunRef.current };
    delete next[runId];
    eventsByRunRef.current = next;
    setEventsByRun(next);
  }, [clearRunHistoryLog]);

  return { eventsByRun, historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskOutput, clearRunOutput };
}
