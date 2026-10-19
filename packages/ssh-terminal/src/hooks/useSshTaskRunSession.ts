import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  cancelSshTaskRun,
  clearSshTaskRuns,
  getSshTask,
  listWorkspaceEnvironments,
  listWorkspaceVariables,
  runSshTask,
  type SshTaskDetail,
  type SshTaskRun,
  type WorkspaceEnvironment,
  type WorkspaceVariable,
} from "@unfour/command-client";
import { useFeedback, useFeedbackErrorHandler, useI18n } from "@unfour/ui";
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
import { useSshTaskRunOutput } from "./useSshTaskRunOutput";

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
  const feedback = useFeedback();
  const { t } = useI18n();
  const showCompletionWarning = (run: SshTaskRun) => {
    if (run.warnings?.includes("ACTIVITY_RECORD_FAILED")) {
      feedback.info(t("feedback.activityRecordFailed"));
    }
  };
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
  const [activeRun, setActiveRunState] = useState<SshTaskRun | null>(null);
  const [activeRunTask, setActiveRunTask] = useState<SshTaskDetail | null>(null);
  const runEnvironmentSyncKeyRef = useRef<string | null>(null);
  const workspaceScope = useMemo(() => ({ workspaceId }), [workspaceId]);
  const mountedScopeRef = useRef<typeof workspaceScope | null>(null);
  const sessionGenerationRef = useRef(0);
  const historySelectionVersionRef = useRef(0);
  const historySelectionTaskIdRef = useRef<string | null>(null);

  useEffect(() => {
    mountedScopeRef.current = workspaceScope;
    sessionGenerationRef.current += 1;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- A run selection belongs to the current workspace.
    setActiveRunState(null);
    setActiveRunTask(null);
    return () => {
      mountedScopeRef.current = null;
      sessionGenerationRef.current += 1;
    };
  }, [workspaceScope]);

  const setActiveRun = useCallback<typeof setActiveRunState>((run) => {
    historySelectionVersionRef.current += 1;
    historySelectionTaskIdRef.current = null;
    setActiveRunState(run);
  }, []);

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

  const {
    eventsByRun, historyLogByRun, historyLogLoading, loadHistoryLog, clearTaskOutput,
  } = useSshTaskRunOutput({ active, activeRunId: activeRun?.id ?? null, handleError, workspaceId });

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
      showCompletionWarning(run);
      queryClient.invalidateQueries({
        queryKey: ["ssh-task-runs", workspaceId, run.taskId],
      });
    },
  });
  const cancelMutation = useMutation({
    mutationFn: () => cancelSshTaskRun({ workspaceId, runId: activeRun!.id }),
    onSuccess: showCompletionWarning,
    onError: (error) => handleError(error, { key: "feedback.ssh.taskCancelFailed" }),
  });
  const clearMutation = useMutation({
    mutationFn: async (taskId: string) => {
      const generation = sessionGenerationRef.current;
      const isCurrent = () => mountedScopeRef.current === workspaceScope && generation === sessionGenerationRef.current;
      try {
        const result = await clearSshTaskRuns({ workspaceId, taskId });
        if (isCurrent()) {
          if (historySelectionTaskIdRef.current === taskId) historySelectionVersionRef.current += 1;
          clearTaskOutput(taskId);
        }
        // The backend mutation still changed this workspace if the view moved.
        queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, taskId] });
        return result;
      } catch (error) {
        if (isCurrent()) handleError(error, { key: "feedback.ssh.taskHistoryClearFailed" });
        throw error;
      }
    },
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
      if (mountedScopeRef.current !== workspaceScope) return;
      const generation = sessionGenerationRef.current;
      const selectionVersion = ++historySelectionVersionRef.current;
      historySelectionTaskIdRef.current = run.taskId;
      const isCurrent = () => mountedScopeRef.current === workspaceScope && generation === sessionGenerationRef.current &&
        selectionVersion === historySelectionVersionRef.current;
      try {
        const detail = await queryClient.fetchQuery({
          queryKey: ["ssh-task", workspaceId, run.taskId],
          queryFn: () => getSshTask(workspaceId, run.taskId),
        });
        if (!isCurrent()) return;
        setActiveRunState(run);
        setActiveRunTask(detail);

        await loadHistoryLog(run.id, run.taskId);
      } catch (error) {
        if (isCurrent()) handleError(error, { key: "feedback.ssh.taskLoadFailed" });
      }
    },
    [handleError, loadHistoryLog, queryClient, workspaceScope, workspaceId],
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
