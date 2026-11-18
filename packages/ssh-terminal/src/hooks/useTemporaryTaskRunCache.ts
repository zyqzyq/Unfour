import { useCallback, useEffect, useRef, type Dispatch, type SetStateAction } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { isTemporarySshTaskRun, listSshTaskRuns, type SshTaskRun } from "@unfour/command-client";

/** Mirror the parent's in-memory lifetime without retaining a target handle. */
export function useTemporaryTaskRunCache(workspaceId: string, clearRunOutput: (runId: string) => void, setActiveRun: Dispatch<SetStateAction<SshTaskRun | null>>) {
  const queryClient = useQueryClient();
  const temporaryRunsRef = useRef(new Map<string, string>());
  const track = useCallback((run: SshTaskRun) => {
    if (run.workspaceId === workspaceId && isTemporarySshTaskRun(run)) temporaryRunsRef.current.set(run.id, run.taskId);
  }, [workspaceId]);
  const contains = useCallback((runId: string) => temporaryRunsRef.current.has(runId), []);
  useEffect(() => {
    const temporaryRuns = temporaryRunsRef.current;
    temporaryRuns.clear();
    let disposed = false;
    let timer: ReturnType<typeof setTimeout>;
    const trackQuery = (query: { queryKey: readonly unknown[]; state: { data: unknown } }) => {
      const [name, workspace, taskId] = query.queryKey;
      if (disposed || query.queryKey.length !== 3 || name !== "ssh-task-runs" || workspace !== workspaceId || typeof taskId !== "string") return;
      const runs = query.state.data as SshTaskRun[] | undefined;
      if (Array.isArray(runs)) runs.forEach((run) => { if (run.taskId === taskId) track(run); });
    };
    // History lists can load without opening a run. They share the same parent
    // lifetime as selected runs and must not remain cached after parent close.
    queryClient.getQueryCache().findAll({ queryKey: ["ssh-task-runs", workspaceId] }).forEach(trackQuery);
    const unsubscribe = queryClient.getQueryCache().subscribe((event) => {
      if (event.type === "updated") trackQuery(event.query);
    });
    const refresh = async () => {
      // Disconnects retain terminal results; explicit parent close removes the
      // backend record. A failed refresh cannot prove that the parent closed.
      for (const taskId of new Set(temporaryRuns.values())) {
        // A run tracked while this request is in flight may not be present in
        // its response yet. Only reconcile observations made before polling.
        const trackedRunIds = [...temporaryRuns]
          .filter(([, owner]) => owner === taskId)
          .map(([runId]) => runId);
        try {
          const runs = await listSshTaskRuns(workspaceId, taskId);
          if (disposed) return;
          const present = new Set(runs.map((run) => run.id));
          if (trackedRunIds.some((runId) => temporaryRuns.get(runId) === taskId && !present.has(runId))) {
            // An older React Query history request must not re-register a run
            // whose absence this newer poll has already confirmed.
            void queryClient.cancelQueries({ queryKey: ["ssh-task-runs", workspaceId, taskId], exact: true });
          }
          for (const runId of trackedRunIds) {
            if (temporaryRuns.get(runId) !== taskId || present.has(runId)) continue;
            temporaryRuns.delete(runId);
            clearRunOutput(runId);
            setActiveRun((current) => current?.id === runId ? null : current);
            queryClient.setQueryData<SshTaskRun[]>(["ssh-task-runs", workspaceId, taskId], (cached) => cached?.filter((run) => run.id !== runId));
          }
        } catch { /* Retain the last in-memory result until absence is known. */ }
      }
      if (!disposed) timer = setTimeout(() => void refresh(), 1_000);
    };
    timer = setTimeout(() => void refresh(), 1_000);
    return () => {
      disposed = true;
      clearTimeout(timer);
      unsubscribe();
      // Cancelling the query prevents a late response from repopulating rows
      // after this workspace's memory-only observations have been discarded.
      void queryClient.cancelQueries({ queryKey: ["ssh-task-runs", workspaceId] });
      temporaryRuns.clear();
      queryClient.setQueriesData<SshTaskRun[]>({ queryKey: ["ssh-task-runs", workspaceId] }, (runs) => runs?.filter((run) => !isTemporarySshTaskRun(run)));
      queryClient.removeQueries({ queryKey: ["ssh-task-temporary-targets", workspaceId] });
    };
  }, [clearRunOutput, queryClient, setActiveRun, track, workspaceId]);

  return { track, contains };
}
