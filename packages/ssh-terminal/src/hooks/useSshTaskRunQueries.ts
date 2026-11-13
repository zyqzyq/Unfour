import { queryOptions, useQuery } from "@tanstack/react-query";
import { listSshTaskRuns, type SshTaskRun } from "@unfour/command-client";
import { useEffect } from "react";

const RUN_REFRESH_MS = 2_000;

function runsOptions(workspaceId: string, taskId: string | null) {
  return queryOptions({
    queryKey: ["ssh-task-runs", workspaceId, taskId],
    queryFn: () => listSshTaskRuns(workspaceId, taskId!),
  });
}

export function useSshTaskRunQueries({
  workspaceId, historyTaskId, selected, active, onSettled, onRemoved,
}: {
  workspaceId: string;
  historyTaskId: string | null;
  selected: SshTaskRun | null;
  active: boolean;
  onSettled: (run: SshTaskRun) => void;
  onRemoved: (run: SshTaskRun) => void;
}) {
  // MCP can own a run in another process. Its exit produces no desktop event.
  const history = useQuery({
    ...runsOptions(workspaceId, historyTaskId),
    enabled: active && !!historyTaskId,
    refetchInterval: (query) => query.state.data?.some((run) => run.status === "running") ? RUN_REFRESH_MS : false,
  });
  const run = selected?.workspaceId === workspaceId ? selected : null;
  const panel = useQuery({
    ...runsOptions(workspaceId, run?.taskId ?? null),
    enabled: active && run?.status === "running" && run.taskId !== historyTaskId,
    select: (runs) => runs.find((row) => row.id === run?.id),
    refetchInterval: (query) => {
      const current = query.state.data?.find((row) => row.id === run?.id) ?? run;
      return current?.status === "running" ? RUN_REFRESH_MS : false;
    },
  });
  // Both observers share the cache when history and output show the same task.
  const completed = run?.status === "running" && panel.data?.status !== "running" ? panel.data : undefined;
  useEffect(() => {
    // Keep the displayed outcome if history is subsequently cleared.
    if (completed) onSettled(completed);
  }, [completed, onSettled]);
  const { data, isSuccess, refetch } = panel;
  useEffect(() => {
    if (!active || run?.status !== "running" || !isSuccess || data) return;
    // A cached list can predate the selected run. Confirm its absence with a
    // new read before closing an output whose history was cleared elsewhere.
    let disposed = false;
    void refetch().then((result) => {
      if (!disposed && result.isSuccess && !result.data) onRemoved(run);
    });
    return () => { disposed = true; };
  }, [active, run, data, isSuccess, refetch, onRemoved]);
  return history;
}
