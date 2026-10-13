import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { readSshTaskRunLog, type SshTaskRunEvent } from "@unfour/command-client";
import type { useFeedbackErrorHandler } from "@unfour/ui";
import { cacheTaskRunLog } from "../model/task-run-events";

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
  const requestsRef = useRef(new Map<symbol, string>());
  const workspaceScope = useMemo(() => ({ workspaceId }), [workspaceId]);
  const mountedScopeRef = useRef<typeof workspaceScope | null>(null);
  const generationRef = useRef(0);

  const clearHistoryLogs = useCallback(() => {
    if (mountedScopeRef.current !== workspaceScope) return false;
    generationRef.current += 1;
    historyLogByRunRef.current = {};
    requestsRef.current.clear();
    setHistoryLogByRun({});
    setLoadingRuns(new Set());
    return true;
  }, [workspaceScope]);

  useEffect(() => {
    mountedScopeRef.current = workspaceScope;
    // Output caches belong to the current workspace lifecycle.
    clearHistoryLogs();
    return () => {
      mountedScopeRef.current = null;
      generationRef.current += 1;
    };
  }, [clearHistoryLogs, workspaceScope]);

  const loadHistoryLog = useCallback(async (runId: string) => {
    if (mountedScopeRef.current !== workspaceScope) return;
    if ((eventsByRunRef.current[runId]?.length ?? 0) > 0 ||
        historyLogByRunRef.current[runId] !== undefined) return;

    const generation = generationRef.current;
    const request = Symbol(runId);
    const isCurrent = () => mountedScopeRef.current === workspaceScope && generation === generationRef.current;
    const cacheLog = (text: string) => {
      if (!isCurrent() || historyLogByRunRef.current[runId] !== undefined) return;
      historyLogByRunRef.current = cacheTaskRunLog(historyLogByRunRef.current, runId, text);
      setHistoryLogByRun(historyLogByRunRef.current);
    };
    requestsRef.current.set(request, runId);
    setLoadingRuns(new Set(requestsRef.current.values()));
    try {
      cacheLog(await readSshTaskRunLog(workspaceId, runId));
    } catch (error) {
      if (!isCurrent()) return;
      handleError(error, { key: "feedback.ssh.taskLogLoadFailed" });
      cacheLog("");
    } finally {
      if (isCurrent()) {
        requestsRef.current.delete(request);
        setLoadingRuns(new Set(requestsRef.current.values()));
      }
    }
  }, [eventsByRunRef, handleError, workspaceScope, workspaceId]);

  return {
    historyLogByRun,
    historyLogLoading: activeRunId !== null && loadingRuns.has(activeRunId),
    loadHistoryLog,
    clearHistoryLogs,
  };
}
