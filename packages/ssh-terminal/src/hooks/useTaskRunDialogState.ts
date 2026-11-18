import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { getSshTask, listSshTaskRuns, listTemporaryRoutes, listWorkspaceEnvironments, listWorkspaceVariables, runSshTask, runTemporarySshTask, isTemporarySshTaskRun,
  type SshTaskDetail, type SshTaskRun, type TemporaryTaskExecutionMode, type WorkspaceEnvironment, type WorkspaceVariable } from "@unfour/command-client";
import { useI18n, type useFeedbackErrorHandler } from "@unfour/ui";
import { detectTaskInputs } from "../model/task-template";
import { workspaceEnvironmentById } from "../model/task-run-inputs";
import { taskInputVersions, taskRunDefaults, taskRunSelection, taskVersion } from "../model/task-run-context";

type DialogState = {
  generation: number;
  detail: SshTaskDetail;
  connectionId: string;
  temporaryHandle: string | null;
  temporaryExecutionMode: TemporaryTaskExecutionMode;
  terminalReady: boolean;
  environmentId: string;
  environments: WorkspaceEnvironment[];
  variables: WorkspaceVariable[];
  values: ReturnType<typeof taskRunDefaults>;
  environmentLoadFailed: boolean;
  notice: string | null;
};

export function useTaskRunDialogState({ workspaceId, handleError, onStarted, onAccepted }: {
  workspaceId: string;
  handleError: ReturnType<typeof useFeedbackErrorHandler>;
  onStarted: (run: SshTaskRun, detail: SshTaskDetail) => void;
  onAccepted?: (run: SshTaskRun) => void;
}) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [state, setState] = useState<DialogState | null>(null);
  const [loading, setLoading] = useState(false);
  const scope = useMemo(() => ({ workspaceId }), [workspaceId]);
  const mounted = useRef<typeof scope | null>(null);
  const generation = useRef(0);
  const starting = useRef<Promise<SshTaskRun> | null>(null);
  const editRevision = useRef(0);
  const restoring = useRef<number | null>(null);
  const temporary = useQuery({
    queryKey: ["ssh-task-temporary-targets", workspaceId],
    queryFn: () => listTemporaryRoutes(workspaceId),
    enabled: !!state && state.detail.task.workspaceId === workspaceId,
    refetchInterval: state ? 1_000 : false,
  });
  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect -- A terminal owner/recovery transition invalidates idle-shell confirmation.
    setState((current) => {
      if (!current?.terminalReady || !current.temporaryHandle) return current;
      const target = temporary.isSuccess && temporary.data.find((entry) => entry.handle === current.temporaryHandle);
      return !target || target.terminalTaskBusy || target.terminalRecoveryRequired ? { ...current, terminalReady: false } : current;
    });
  }, [temporary.data, temporary.isSuccess]);
  useEffect(() => {
    mounted.current = scope;
    generation.current += 1;
    // eslint-disable-next-line react-hooks/set-state-in-effect -- Dialog values belong to one workspace only.
    setState(null);
    editRevision.current = 0;
    restoring.current = null;
    setLoading(false);
    return () => { mounted.current = null; generation.current += 1; };
  }, [scope]);

  const closeRunDialog = useCallback(() => {
    generation.current += 1;
    setState(null);
    editRevision.current = 0;
    restoring.current = null;
    setLoading(false);
  }, []);

  const runMutation = useMutation({
    mutationFn: () => {
      if (starting.current) return starting.current;
      if (!state || state.generation !== generation.current || mounted.current !== scope || loading) return Promise.reject(new Error(t("ssh.tasks.run.prepareAgain")));
      const selected = state;
      if (selected.temporaryHandle && selected.temporaryExecutionMode === "terminal") {
        if (!selected.terminalReady) return Promise.reject(new Error(t("ssh.tasks.run.terminalReadyRequired")));
        // Consume confirmation before rechecking the live session. A failed start
        // can leave the dialog open after an ownership or connection change.
        setState((current) => current === selected ? { ...current, terminalReady: false } : current);
        if (selected.detail.steps.some((step) => step.enabled && step.stepType !== "command")) return Promise.reject(new Error(t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_COMMAND_ONLY")));
      }
      const version = generation.current;
      const current = () => mounted.current === scope && generation.current === version;
      const environment = workspaceEnvironmentById(selected.environments, selected.environmentId);
      if (selected.environmentLoadFailed || (selected.environmentId && !environment)) return Promise.reject(new Error(t("ssh.tasks.run.environmentMissing")));
      const input = {
        workspaceId, taskId: selected.detail.task.id, connectionId: selected.temporaryHandle ? null : selected.connectionId || null,
        inputs: Object.fromEntries(Object.entries(selected.values.inputs).map(([name, value]) => [name, selected.values.sources[name] === "manual" ? value : ""])), secretInputNames: selected.values.secretNames,
        context: { environmentId: selected.environmentId || null, taskVersion: taskVersion(selected.detail),
          environmentUpdatedAt: environment?.updatedAt ?? null, inputSources: selected.values.sources,
          inputVersions: taskInputVersions(selected.values.sources, selected.variables, environment) },
      };
      const execute = async () => {
        if (!selected.temporaryHandle) return runSshTask(input);
        // Re-read at dispatch: a cached option cannot authorize a closed or
        // switched-workspace target, and it must never fall back to a default.
        const live = await listTemporaryRoutes(workspaceId);
        if (!current()) throw new Error(t("ssh.tasks.run.prepareAgain"));
        const target = live.find((target) => target.handle === selected.temporaryHandle);
        if (!target) throw new Error(t("network.errors.SSH_TUNNEL_TEMPORARY_UNAVAILABLE"));
        if (target.terminalTaskBusy) throw new Error(t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_BUSY"));
        if (selected.temporaryExecutionMode === "terminal" && target.terminalRecoveryRequired) throw new Error(t("ssh.tasks.run.errors.SSH_TASK_TERMINAL_RECOVERY_REQUIRED"));
        return runTemporarySshTask(input, selected.temporaryHandle, selected.temporaryExecutionMode);
      };
      const operation = execute().then((run) => {
        void queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, run.taskId] });
        // Closing the dialog changes selection, not the lifetime of a start
        // already accepted by the service. Its output still needs cleanup.
        if (mounted.current === scope && run.workspaceId === workspaceId) onAccepted?.(run);
        if (current()) { closeRunDialog(); onStarted(run, selected.detail); }
        return run;
      }).finally(() => { if (starting.current === operation) starting.current = null; });
      starting.current = operation;
      return operation;
    },
  });
  const resetMutation = runMutation.reset;

  const prepareRun = useCallback(async (taskId: string, previous?: SshTaskRun | "latest") => {
    if (mounted.current !== scope || starting.current) return;
    if (!matchesRunScope(previous, workspaceId, taskId)) return;
    if (previous === "latest") {
      if (restoring.current !== null) return;
      if (editRevision.current > 0) {
        setState((value) => value ? { ...value, notice: "ssh.tasks.run.editsKept" } : value);
        return;
      }
    } else { editRevision.current = 0; restoring.current = null; }
    const version = ++generation.current;
    const revision = editRevision.current;
    if (previous === "latest") restoring.current = version;
    const current = () => mounted.current === scope && generation.current === version;
    setLoading(true);
    if (previous !== "latest") setState(null);
    resetMutation();
    try {
      const detail = await queryClient.fetchQuery({ queryKey: ["ssh-task", workspaceId, taskId], meta: { cloudSyncWorkspaceId: workspaceId, localDataDomains: ["ssh-tasks"] }, queryFn: () => getSshTask(workspaceId, taskId), staleTime: 0 });
      const run = previous === "latest" ? (await listSshTaskRuns(workspaceId, taskId))[0] : previous;
      if (previous === "latest" && !run) {
        if (current()) setState((value) => value ? { ...value, generation: version, notice: "ssh.tasks.run.noPrevious" } : value);
        return;
      }
      let variables: WorkspaceVariable[] = [];
      let environments: WorkspaceEnvironment[] = [];
      let environmentLoadFailed = false;
      try {
        [variables, environments] = await Promise.all([
          listWorkspaceVariables(workspaceId), listWorkspaceEnvironments(workspaceId),
        ]);
      } catch { environmentLoadFailed = true; }
      if (!current()) return;
      if (previous === "latest" && editRevision.current !== revision) {
        setState((value) => value ? { ...value, generation: version, notice: "ssh.tasks.run.editsKept" } : value);
        return;
      }
      environments = environments.filter((environment) => !environment.deletedAt);
      setState({ generation: version, detail, environments, variables, environmentLoadFailed,
        ...taskRunSelection(detail, variables, environments, run && isTemporarySshTaskRun(run) ? undefined : run), temporaryHandle: null,
        temporaryExecutionMode: "terminal", terminalReady: false,
        ...(run && isTemporarySshTaskRun(run) ? { connectionId: "", notice: "ssh.tasks.run.temporaryReselect" } : {}) });
    } catch (error) {
      if (current()) {
        setState((value) => previous === "latest" && value ? { ...value, generation: version } : null);
        handleError(error, { key: "feedback.ssh.taskLoadFailed" });
      }
    } finally {
      if (restoring.current === version) restoring.current = null;
      if (current()) setLoading(false);
    }
  }, [handleError, queryClient, resetMutation, scope, workspaceId]);

  const changeRunEnvironment = useCallback(async (environmentId: string) => {
    if (!state || starting.current) return;
    editRevision.current += 1;
    // The pending refresh must not restore a readiness confirmation invalidated
    // by a terminal owner/recovery transition while it was in flight.
    const selected = { ...state, terminalReady: false };
    const version = ++generation.current;
    const current = () => mounted.current === scope && generation.current === version;
    setLoading(true);
    setState(selected);
    try {
      const [variables, environments] = await Promise.all([listWorkspaceVariables(workspaceId), listWorkspaceEnvironments(workspaceId)]);
      if (!current()) return;
      const environment = workspaceEnvironmentById(environments, environmentId);
      setState({ ...selected, generation: version, environmentId, variables, environments: environments.filter((env) => !env.deletedAt), environmentLoadFailed: false,
        values: taskRunDefaults(detectTaskInputs(selected.detail.steps, true), variables, environment) });
    } catch (error) {
      if (current()) { setState({ ...selected, generation: version, environmentLoadFailed: true }); handleError(error, { key: "feedback.ssh.taskLoadFailed" }); }
    } finally { if (current()) setLoading(false); }
  }, [handleError, scope, state, workspaceId]);

  const setRunInputs = (update: Record<string, string> | ((values: Record<string, string>) => Record<string, string>)) => {
    if (starting.current || (state?.generation !== generation.current && restoring.current === null)) return;
    editRevision.current += 1;
    setState((current) => {
      if (!current) return current;
      const inputs = typeof update === "function" ? update(current.values.inputs) : update;
      const sources = { ...current.values.sources };
      for (const name of Object.keys(inputs)) if (inputs[name] !== current.values.inputs[name]) sources[name] = "manual";
      return { ...current, values: { ...current.values, inputs, sources } };
    });
  };

  const toggleSecretInput = (name: string) => {
    if (starting.current || (state?.generation !== generation.current && restoring.current === null)) return;
    editRevision.current += 1;
    setState((current) => {
      if (!current || current.values.automaticSecretNames.includes(name)) return current;
      const secretNames = current.values.secretNames.includes(name) ? current.values.secretNames.filter((key) => key !== name) : [...current.values.secretNames, name];
      return { ...current, values: { ...current.values, secretNames } };
    });
  };

  const {
    detail = null, connectionId = "", temporaryHandle = null, temporaryExecutionMode = "terminal", terminalReady = false, environmentId = "", environments = [],
    environmentLoadFailed = false, notice = null,
    values = { inputs: {}, secretNames: [], automaticSecretNames: [], filledFromWorkspace: [] },
  } = state ?? {};
  const environment = workspaceEnvironmentById(environments, environmentId);
  return {
    prepareRun, closeRunDialog, changeRunEnvironment, runMutation, setRunInputs, toggleSecretInput,
    restorePreviousRun: () => state && prepareRun(state.detail.task.id, "latest"),
    runPreparing: loading,
    runNotice: notice ? t(notice) : null,
    runDialogTask: detail,
    runConnectionId: connectionId,
    runTemporaryHandle: temporaryHandle,
    runTemporaryExecutionMode: temporaryExecutionMode,
    runTerminalReady: terminalReady,
    setRunTemporaryExecutionMode: (mode: TemporaryTaskExecutionMode) => {
      if (starting.current || (state?.generation !== generation.current && restoring.current === null)) return;
      editRevision.current += 1;
      setState((current) => current ? { ...current, temporaryExecutionMode: mode, terminalReady: false } : current);
    },
    setRunTerminalReady: (ready: boolean) => {
      if (starting.current || (state?.generation !== generation.current && restoring.current === null)) return;
      editRevision.current += 1;
      setState((current) => current ? { ...current, terminalReady: ready } : current);
    },
    runTemporarySessions: temporary.isSuccess ? temporary.data : [],
    runTemporaryLoading: temporary.isFetching,
    setRunTemporaryHandle: (handle: string) => {
      if (starting.current || (state?.generation !== generation.current && restoring.current === null)) return;
      editRevision.current += 1;
      setState((current) => current ? { ...current, temporaryHandle: handle || null, connectionId: "", temporaryExecutionMode: "terminal", terminalReady: false } : current);
    },
    setRunConnectionId: (connectionId: string) => {
      if (starting.current || (state?.generation !== generation.current && restoring.current === null)) return;
      editRevision.current += 1;
      setState((current) => current ? { ...current, connectionId, temporaryHandle: null, temporaryExecutionMode: "terminal", terminalReady: false } : current);
    },
    runEnvironmentId: environmentId,
    runEnvironments: environments,
    runEnvironmentLoadFailed: environmentLoadFailed,
    runActiveEnvironmentName: environment?.name ?? null,
    runInputs: values.inputs,
    runSecretInputs: values.secretNames,
    runAutomaticSecretInputs: values.automaticSecretNames,
    runInputScopeKey: state?.generation ?? 0,
    runFilledFromWorkspace: values.filledFromWorkspace.length > 0,
  };
}

function matchesRunScope(previous: SshTaskRun | "latest" | undefined, workspaceId: string, taskId: string) {
  return !previous || previous === "latest" || (previous.workspaceId === workspaceId && previous.taskId === taskId);
}
