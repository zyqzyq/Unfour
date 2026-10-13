import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import {
  registerSshTaskRunChannel,
  type SshTaskRunEvent,
} from "@unfour/command-client";
import type { useFeedbackErrorHandler } from "@unfour/ui";
import {
  appendTaskRunEventCache,
  removeTaskRunEventsForTask,
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
  const [eventsByRun, setEventsByRun] = useState<Record<string, SshTaskRunEvent[]>>({});
  const eventsByRunRef = useRef(eventsByRun);
  const pendingEventsRef = useRef<SshTaskRunEvent[]>([]);
  const tasksSurfaceActiveRef = useRef(active);
  const activeRunIdRef = useRef(activeRunId);
  const { historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskHistoryLogs } =
    useSshTaskRunHistory({ activeRunId, eventsByRunRef, handleError, workspaceId });

  useEffect(() => {
    tasksSurfaceActiveRef.current = active;
    activeRunIdRef.current = activeRunId;
  }, [active, activeRunId]);

  useEffect(() => {
    let disposed = false;
    let dispose: (() => void) | null = null;
    eventsByRunRef.current = {};
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Live output is scoped to this workspace subscription.
    setEventsByRun({});
    // Coalesce per-line task output into one React update ~per frame. Without
    // this, a verbose command (or transfer progress) re-renders thousands of
    // transcript spans and can freeze the Tasks surface.
    pendingEventsRef.current = [];
    let flushTimer: ReturnType<typeof setTimeout> | null = null;
    const flushPending = () => {
      flushTimer = null;
      if (disposed) {
        return;
      }
      if (!pendingEventsRef.current.length) return;
      // Keep buffering while Connections is shown so a hidden Tasks tree does
      // not re-render on every remote line; flush when Tasks becomes active.
      if (!tasksSurfaceActiveRef.current) {
        flushTimer = setTimeout(flushPending, 250);
        return;
      }
      const batch = pendingEventsRef.current;
      pendingEventsRef.current = [];
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
    registerSshTaskRunChannel((event) => {
      if (disposed) return;
      pendingEventsRef.current.push(event);
      if (pendingEventsRef.current.length > 10_000) {
        pendingEventsRef.current = pendingEventsRef.current.slice(-10_000);
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
      pendingEventsRef.current = [];
      dispose?.();
    };
  }, [queryClient, workspaceId]);

  const clearTaskOutput = useCallback((taskId: string) => {
    if (!clearTaskHistoryLogs(taskId)) return;
    // Cleanup success is the boundary for events already received. Do not block
    // run IDs: a running task can still produce fresh output after cleanup.
    pendingEventsRef.current = pendingEventsRef.current.filter((event) => event.taskId !== taskId);
    eventsByRunRef.current = removeTaskRunEventsForTask(eventsByRunRef.current, taskId);
    setEventsByRun(eventsByRunRef.current);
  }, [clearTaskHistoryLogs]);

  return { eventsByRun, historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskOutput };
}
