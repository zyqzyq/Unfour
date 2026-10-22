import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { getSshTask, listSshTaskRuns, listWorkspaceEnvironments, listWorkspaceVariables, runSshTask,
  type SshTaskDetail, type SshTaskRun, type WorkspaceEnvironment, type WorkspaceVariable } from "@unfour/command-client";
import { useI18n, type useFeedbackErrorHandler } from "@unfour/ui";
import { detectTaskInputs } from "../model/task-template";
import { workspaceEnvironmentById } from "../model/task-run-inputs";
import { sensitiveTaskInput, taskInputVersions, taskRunDefaults, taskRunSelection, taskVersion } from "../model/task-run-context";

type DialogState = {
  generation: number;
  detail: SshTaskDetail;
  connectionId: string;
  environmentId: string;
  environments: WorkspaceEnvironment[];
  variables: WorkspaceVariable[];
  values: ReturnType<typeof taskRunDefaults>;
  environmentLoadFailed: boolean;
  notice: string | null;
};

export function useTaskRunDialogState({ workspaceId, handleError, onStarted }: {
  workspaceId: string;
  handleError: ReturnType<typeof useFeedbackErrorHandler>;
  onStarted: (run: SshTaskRun, detail: SshTaskDetail) => void;
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
      const version = generation.current;
      const current = () => mounted.current === scope && generation.current === version;
      const environment = workspaceEnvironmentById(selected.environments, selected.environmentId);
      if (selected.environmentLoadFailed || (selected.environmentId && !environment)) return Promise.reject(new Error(t("ssh.tasks.run.environmentMissing")));
      const operation = runSshTask({
        workspaceId, taskId: selected.detail.task.id, connectionId: selected.connectionId || null,
        inputs: Object.fromEntries(Object.entries(selected.values.inputs).map(([name, value]) => [name, selected.values.sources[name] === "manual" ? value : ""])), secretInputNames: selected.values.secretNames,
        context: { environmentId: selected.environmentId || null, taskVersion: taskVersion(selected.detail),
          environmentUpdatedAt: environment?.updatedAt ?? null, inputSources: selected.values.sources,
          inputVersions: taskInputVersions(selected.values.sources, selected.variables, environment) },
      }).then((run) => {
        void queryClient.invalidateQueries({ queryKey: ["ssh-task-runs", workspaceId, run.taskId] });
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
      const detail = await queryClient.fetchQuery({ queryKey: ["ssh-task", workspaceId, taskId], queryFn: () => getSshTask(workspaceId, taskId), staleTime: 0 });
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
        ...taskRunSelection(detail, variables, environments, run) });
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
    const selected = state;
    const version = ++generation.current;
    const current = () => mounted.current === scope && generation.current === version;
    setLoading(true);
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
    editRevision.current += 1;
    setState((current) => {
      if (!current || sensitiveTaskInput(name)) return current;
      const defaults = taskRunDefaults([name], current.variables, workspaceEnvironmentById(current.environments, current.environmentId));
      if (defaults.secretNames.includes(name)) return current;
      const secretNames = current.values.secretNames.includes(name) ? current.values.secretNames.filter((key) => key !== name) : [...current.values.secretNames, name];
      return { ...current, values: { ...current.values, secretNames } };
    });
  };

  const {
    detail = null, connectionId = "", environmentId = "", environments = [],
    environmentLoadFailed = false, notice = null,
    values = { inputs: {}, secretNames: [], filledFromWorkspace: [] },
  } = state ?? {};
  const environment = workspaceEnvironmentById(environments, environmentId);
  return {
    prepareRun, closeRunDialog, changeRunEnvironment, runMutation, setRunInputs, toggleSecretInput,
    restorePreviousRun: () => state && prepareRun(state.detail.task.id, "latest"),
    runPreparing: loading,
    runNotice: notice ? t(notice) : null,
    runDialogTask: detail,
    runConnectionId: connectionId,
    setRunConnectionId: (connectionId: string) => {
      editRevision.current += 1;
      setState((current) => current ? { ...current, connectionId } : current);
    },
    runEnvironmentId: environmentId,
    runEnvironments: environments,
    runEnvironmentLoadFailed: environmentLoadFailed,
    runActiveEnvironmentName: environment?.name ?? null,
    runInputs: values.inputs,
    runSecretInputs: values.secretNames,
    runFilledFromWorkspace: values.filledFromWorkspace.length > 0,
  };
}

function matchesRunScope(previous: SshTaskRun | "latest" | undefined, workspaceId: string, taskId: string) {
  return !previous || previous === "latest" || (previous.workspaceId === workspaceId && previous.taskId === taskId);
}
