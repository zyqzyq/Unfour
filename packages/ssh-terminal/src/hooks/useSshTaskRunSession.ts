import { useCallback, useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  cancelSshTaskRun,
  clearSshTaskRuns,
  getSshTask,
  listWorkspaceEnvironments,
  listWorkspaceVariables,
  readSshTaskRunLog,
  registerSshTaskRunChannel,
  runSshTask,
  type SshTaskDetail,
  type SshTaskRun,
  type SshTaskRunEvent,
  type WorkspaceEnvironment,
  type WorkspaceVariable,
} from "@unfour/command-client";
import { useFeedbackErrorHandler } from "@unfour/ui";
import {
  detectTaskInputs,
  preferredTaskConnectionId,
} from "../model/task-template";
import {
  activeWorkspaceEnvironmentId,
  activeWorkspaceEnvironmentName,
  defaultTaskRunInputs,
  mergeWorkspaceVariables,
  workspaceEnvironmentById,
} from "../model/task-run-inputs";
import {
  appendTaskRunEventCache,
  cacheTaskRunLog,
  removeTaskRunEventsForTask,
} from "../model/task-run-events";

export function useSshTaskRunSession({
  active,
  handleError,
  workspaceId,
}: {
  active: boolean;
  handleError: ReturnType<typeof useFeedbackErrorHandler>;
  workspaceId: string;
}) {
  const queryClient = useQueryClient();
  const [runDialogTask, setRunDialogTask] = useState<SshTaskDetail | null>(null);
  const [runConnectionId, setRunConnectionId] = useState("");
  const [runInputs, setRunInputs] = useState<Record<string, string>>({});
  const [runSecretInputs, setRunSecretInputs] = useState<string[]>([]);
  const [runFilledFromWorkspace, setRunFilledFromWorkspace] = useState(false);
  const [runEnvironmentId, setRunEnvironmentId] = useState("");
  const [runEnvironments, setRunEnvironments] = useState<WorkspaceEnvironment[]>([]);
  const [runWorkspaceVariables, setRunWorkspaceVariables] = useState<WorkspaceVariable[]>([]);
  const [runEnvironmentLoadFailed, setRunEnvironmentLoadFailed] = useState(false);
  const [runActiveEnvironmentName, setRunActiveEnvironmentName] = useState<string | null>(
    null,
  );
  const [activeRun, setActiveRun] = useState<SshTaskRun | null>(null);
  const [activeRunTask, setActiveRunTask] = useState<SshTaskDetail | null>(null);
  const [eventsByRun, setEventsByRun] = useState<Record<string, SshTaskRunEvent[]>>({});
  const [historyLogByRun, setHistoryLogByRun] = useState<Record<string, string>>({});
  const [historyLogLoading, setHistoryLogLoading] = useState(false);
  const eventsByRunRef = useRef(eventsByRun);
  const historyLogByRunRef = useRef(historyLogByRun);
  const runEnvironmentSyncKeyRef = useRef<string | null>(null);
  const tasksSurfaceActiveRef = useRef(active);
  const activeRunIdRef = useRef(activeRun?.id ?? null);

  useEffect(() => {
    eventsByRunRef.current = eventsByRun;
    historyLogByRunRef.current = historyLogByRun;
    tasksSurfaceActiveRef.current = active;
    activeRunIdRef.current = activeRun?.id ?? null;
  }, [active, activeRun?.id, eventsByRun, historyLogByRun]);

  const workspaceEnvironmentsQuery = useQuery({
    enabled: Boolean(active && workspaceId),
    queryKey: ["workspace-environments", workspaceId],
    queryFn: () => listWorkspaceEnvironments(workspaceId),
    staleTime: 0,
  });

  useEffect(() => {
    if (!runDialogTask) {
      runEnvironmentSyncKeyRef.current = null;
      return;
    }

    const environments = workspaceEnvironmentsQuery.data ?? runEnvironments;
    if (workspaceEnvironmentsQuery.data) {
      // eslint-disable-next-line react-hooks/set-state-in-effect -- Keep the open run dialog's environment picker in sync with the shared query cache.
      setRunEnvironments(environments);
    }

    const detectedInputs = detectTaskInputs(runDialogTask.steps, true);
    const environmentId = activeWorkspaceEnvironmentId(environments);
    const activeEnvironment = workspaceEnvironmentById(environments, environmentId);
    const syncKey = [
      runDialogTask.task.id,
      detectedInputs.join("\u0000"),
      environmentId,
      activeEnvironment?.updatedAt ?? "",
      activeEnvironment?.revision ?? "",
    ].join("\u0001");
    if (runEnvironmentSyncKeyRef.current === syncKey) return;
    runEnvironmentSyncKeyRef.current = syncKey;

    const defaults = defaultTaskRunInputs(
      detectedInputs,
      mergeWorkspaceVariables(runWorkspaceVariables, activeEnvironment),
    );
    const filledInputNames = new Set(defaults.filledFromWorkspace);
    setRunEnvironmentId(environmentId);
    setRunInputs((current) =>
      Object.fromEntries(
        detectedInputs.map((name) => [
          name,
          filledInputNames.has(name) ? defaults.inputs[name] ?? "" : current[name] ?? "",
        ]),
      ),
    );
    setRunSecretInputs(defaults.secretNames);
    setRunFilledFromWorkspace(defaults.filledFromWorkspace.length > 0);
    setRunActiveEnvironmentName(activeWorkspaceEnvironmentName(environments));
  }, [
    runDialogTask,
    runEnvironments,
    runWorkspaceVariables,
    workspaceEnvironmentsQuery.data,
  ]);

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

  const runMutation = useMutation({
    mutationFn: () =>
      runSshTask({
        workspaceId,
        taskId: runDialogTask!.task.id,
        connectionId: runConnectionId || null,
        inputs: runInputs,
        secretInputNames: runSecretInputs,
      }),
    onSuccess: (run) => {
      setActiveRun(run);
      setActiveRunTask(runDialogTask);
      setRunDialogTask(null);
      queryClient.invalidateQueries({
        queryKey: ["ssh-task-runs", workspaceId, run.taskId],
      });
    },
  });
  const cancelMutation = useMutation({
    mutationFn: () => cancelSshTaskRun({ workspaceId, runId: activeRun!.id }),
    onError: (error) => handleError(error, { key: "feedback.ssh.taskCancelFailed" }),
  });
  const clearMutation = useMutation({
    mutationFn: (taskId: string) => clearSshTaskRuns({ workspaceId, taskId }),
    onSuccess: (_, taskId) => {
      setEventsByRun((current) => removeTaskRunEventsForTask(current, taskId));
      setHistoryLogByRun({});
      queryClient.invalidateQueries({
        queryKey: ["ssh-task-runs", workspaceId, taskId],
      });
    },
    onError: (error) =>
      handleError(error, { key: "feedback.ssh.taskHistoryClearFailed" }),
  });
  const resetRunMutation = runMutation.reset;

  const prepareRun = useCallback(
    async (taskId: string) => {
      try {
        const detail = await queryClient.fetchQuery({
          queryKey: ["ssh-task", workspaceId, taskId],
          queryFn: () => getSshTask(workspaceId, taskId),
        });
        const detectedInputs = detectTaskInputs(detail.steps, true);
        let inputs = Object.fromEntries(detectedInputs.map((name) => [name, ""]));
        let secretNames: string[] = [];
        let filledFromWorkspace = false;
        let activeEnvironmentName: string | null = null;
        let environmentId = "";
        let workspaceVariables: WorkspaceVariable[] = [];
        let environments: WorkspaceEnvironment[] = [];
        let environmentLoadFailed = false;

        try {
          [workspaceVariables, environments] = await Promise.all([
            queryClient.fetchQuery({
              queryKey: ["workspace-variables", workspaceId],
              queryFn: () => listWorkspaceVariables(workspaceId),
              staleTime: 0,
            }),
            queryClient.fetchQuery({
              queryKey: ["workspace-environments", workspaceId],
              queryFn: () => listWorkspaceEnvironments(workspaceId),
              staleTime: 0,
            }),
          ]);
          environmentId = activeWorkspaceEnvironmentId(environments);
          const defaults = defaultTaskRunInputs(
            detectedInputs,
            mergeWorkspaceVariables(
              workspaceVariables,
              workspaceEnvironmentById(environments, environmentId),
            ),
          );
          inputs = defaults.inputs;
          secretNames = defaults.secretNames;
          filledFromWorkspace = defaults.filledFromWorkspace.length > 0;
          activeEnvironmentName = activeWorkspaceEnvironmentName(environments);
        } catch {
          // Workspace defaults are optional; keep empty inputs if they fail to load.
          environmentLoadFailed = true;
        }

        setRunDialogTask(detail);
        setRunConnectionId(preferredTaskConnectionId(detail.localBinding));
        setRunInputs(inputs);
        setRunSecretInputs(secretNames);
        setRunFilledFromWorkspace(filledFromWorkspace);
        setRunActiveEnvironmentName(activeEnvironmentName);
        setRunEnvironmentId(environmentId);
        setRunEnvironments(environments);
        setRunWorkspaceVariables(workspaceVariables);
        setRunEnvironmentLoadFailed(environmentLoadFailed);
        resetRunMutation();
      } catch (error) {
        handleError(error, { key: "feedback.ssh.taskLoadFailed" });
      }
    },
    [handleError, queryClient, resetRunMutation, workspaceId],
  );

  const changeRunEnvironment = useCallback(
    (environmentId: string) => {
      if (!runDialogTask) return;
      const defaults = defaultTaskRunInputs(
        detectTaskInputs(runDialogTask.steps, true),
        mergeWorkspaceVariables(
          runWorkspaceVariables,
          workspaceEnvironmentById(runEnvironments, environmentId),
        ),
      );
      const environment = workspaceEnvironmentById(runEnvironments, environmentId);
      setRunEnvironmentId(environmentId);
      setRunInputs(defaults.inputs);
      setRunSecretInputs(defaults.secretNames);
      setRunFilledFromWorkspace(defaults.filledFromWorkspace.length > 0);
      setRunActiveEnvironmentName(environment?.name?.trim() || null);
    },
    [runDialogTask, runEnvironments, runWorkspaceVariables],
  );

  const openHistoryRun = useCallback(
    async (run: SshTaskRun) => {
      try {
        const detail = await queryClient.fetchQuery({
          queryKey: ["ssh-task", workspaceId, run.taskId],
          queryFn: () => getSshTask(workspaceId, run.taskId),
        });
        setActiveRun(run);
        setActiveRunTask(detail);

        const hasLiveEvents = (eventsByRunRef.current[run.id]?.length ?? 0) > 0;
        if (hasLiveEvents || historyLogByRunRef.current[run.id] !== undefined) {
          return;
        }

        setHistoryLogLoading(true);
        try {
          const logText = await readSshTaskRunLog(workspaceId, run.id);
          setHistoryLogByRun((current) =>
            current[run.id] === undefined
              ? cacheTaskRunLog(current, run.id, logText)
              : current,
          );
        } catch (error) {
          handleError(error, { key: "feedback.ssh.taskLogLoadFailed" });
          setHistoryLogByRun((current) =>
            current[run.id] === undefined
              ? cacheTaskRunLog(current, run.id, "")
              : current,
          );
        } finally {
          setHistoryLogLoading(false);
        }
      } catch (error) {
        handleError(error, { key: "feedback.ssh.taskLoadFailed" });
      }
    },
    [handleError, queryClient, workspaceId],
  );

  return {
    activeRun,
    activeRunTask,
    cancelMutation,
    changeRunEnvironment,
    clearMutation,
    eventsByRun,
    historyLogByRun,
    historyLogLoading,
    openHistoryRun,
    prepareRun,
    runActiveEnvironmentName,
    runConnectionId,
    runDialogTask,
    runEnvironmentId,
    runEnvironmentLoadFailed,
    runEnvironments,
    runFilledFromWorkspace,
    runInputs,
    runMutation,
    runSecretInputs,
    setActiveRun,
    setActiveRunTask,
    setRunConnectionId,
    setRunDialogTask,
    setRunInputs,
  };
}
