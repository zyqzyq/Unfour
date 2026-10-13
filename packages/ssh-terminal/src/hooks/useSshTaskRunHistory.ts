import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { readSshTaskRunLog, type SshTaskRunEvent } from "@unfour/command-client";
import type { useFeedbackErrorHandler } from "@unfour/ui";
import { cacheTaskRunLog } from "../model/task-run-events";

type HistoryRequest = { runId: string; taskId: string };

function historyLoadingRuns(requests: ReadonlyMap<symbol, HistoryRequest>) {
  return new Set([...requests.values()].map((request) => request.runId));
}

/** Drop ownership for logs evicted by the shared four-run bound. */
function retainCachedRunTasks(runTaskIds: Map<string, string>, logs: Record<string, string>) {
  for (const runId of [...runTaskIds.keys()]) {
    if (logs[runId] === undefined) runTaskIds.delete(runId);
  }
}

/** Invalidate history reads without cancelling backend work or changing cache bounds. */
export function useSshTaskRunHistory({
  activeRunId,
  eventsByRunRef,
  handleError,
  workspaceId,
}: {
  activeRunId: string | null;
  eventsByRunRef: { current: Record<string, SshTaskRunEvent[]> };
  handleError: ReturnType<typeof useFeedbackErrorHandler>;
  workspaceId: string;
}) {
  const [historyLogByRun, setHistoryLogByRun] = useState<Record<string, string>>({});
  const [loadingRuns, setLoadingRuns] = useState<Set<string>>(new Set());
  const historyLogByRunRef = useRef<Record<string, string>>({});
  const runTaskIdsRef = useRef(new Map<string, string>());
  const requestsRef = useRef(new Map<symbol, HistoryRequest>());
  const taskGenerationsRef = useRef(new Map<string, number>());
  const workspaceScope = useMemo(() => ({ workspaceId }), [workspaceId]);
  const mountedScopeRef = useRef<typeof workspaceScope | null>(null);
  const generationRef = useRef(0);

  const resetHistoryLogs = useCallback(() => {
    if (mountedScopeRef.current !== workspaceScope) return false;
    generationRef.current += 1;
    taskGenerationsRef.current.clear();
    historyLogByRunRef.current = {};
    runTaskIdsRef.current.clear();
    requestsRef.current.clear();
    setHistoryLogByRun({});
    setLoadingRuns(new Set());
    return true;
  }, [workspaceScope]);

  const clearTaskHistoryLogs = useCallback((taskId: string) => {
    if (mountedScopeRef.current !== workspaceScope) return false;
    // Do not advance the workspace generation. That would discard every task's
    // in-flight read, including one the user is still viewing.
    taskGenerationsRef.current.set(taskId, (taskGenerationsRef.current.get(taskId) ?? 0) + 1);
    const nextLogs = { ...historyLogByRunRef.current };
    for (const [runId, owner] of [...runTaskIdsRef.current]) {
      if (owner !== taskId) continue;
      delete nextLogs[runId];
      runTaskIdsRef.current.delete(runId);
    }
    historyLogByRunRef.current = nextLogs;
    for (const [request, meta] of [...requestsRef.current]) {
      if (meta.taskId === taskId) requestsRef.current.delete(request);
    }
    setHistoryLogByRun(nextLogs);
    setLoadingRuns(historyLoadingRuns(requestsRef.current));
    return true;
  }, [workspaceScope]);

  useEffect(() => {
    mountedScopeRef.current = workspaceScope;
    // Output caches belong to the current workspace lifecycle.
    resetHistoryLogs();
    return () => {
      mountedScopeRef.current = null;
      generationRef.current += 1;
    };
  }, [resetHistoryLogs, workspaceScope]);

  const loadHistoryLog = useCallback(async (runId: string, taskId: string) => {
    if (mountedScopeRef.current !== workspaceScope) return;
    if ((eventsByRunRef.current[runId]?.length ?? 0) > 0 ||
        historyLogByRunRef.current[runId] !== undefined) return;

    const generation = generationRef.current;
    const taskGeneration = taskGenerationsRef.current.get(taskId) ?? 0;
    const request = Symbol(runId);
    const isCurrent = () => mountedScopeRef.current === workspaceScope &&
      generation === generationRef.current &&
      (taskGenerationsRef.current.get(taskId) ?? 0) === taskGeneration;
    const cacheLog = (text: string) => {
      if (!isCurrent() || historyLogByRunRef.current[runId] !== undefined) return;
      historyLogByRunRef.current = cacheTaskRunLog(historyLogByRunRef.current, runId, text);
      runTaskIdsRef.current.set(runId, taskId);
      retainCachedRunTasks(runTaskIdsRef.current, historyLogByRunRef.current);
      setHistoryLogByRun(historyLogByRunRef.current);
    };
    requestsRef.current.set(request, { runId, taskId });
    setLoadingRuns(historyLoadingRuns(requestsRef.current));
    try {
      cacheLog(await readSshTaskRunLog(workspaceId, runId));
    } catch (error) {
      if (!isCurrent()) return;
      handleError(error, { key: "feedback.ssh.taskLogLoadFailed" });
      cacheLog("");
    } finally {
      if (isCurrent()) {
        requestsRef.current.delete(request);
        setLoadingRuns(historyLoadingRuns(requestsRef.current));
      }
    }
  }, [eventsByRunRef, handleError, workspaceScope, workspaceId]);

  return {
    historyLogByRun,
    historyLogLoading: activeRunId !== null && loadingRuns.has(activeRunId),
    loadHistoryLog,
    clearTaskHistoryLogs,
  };
}
