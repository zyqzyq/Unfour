import type { SshTaskDetail, SshTaskInputSource, SshTaskRun, SshTaskRunContextInput } from "@unfour/command-client";
import { activeWorkspaceEnvironmentId, defaultTaskRunInputs, mergeWorkspaceVariables, workspaceEnvironmentById, type WorkspaceEnvironmentLike, type WorkspaceVariableLike } from "./task-run-inputs";
import { detectTaskInputs, preferredTaskConnectionId } from "./task-template";

export function taskVersion(detail: SshTaskDetail) {
  return [detail.task.updatedAt, ...detail.steps.map((step) => `${step.id}:${step.updatedAt}`)].join("|");
}

// Mirrors the shared Rust sensitive-name boundary; Rust remains authoritative.
export function sensitiveTaskInput(name: string) {
  return /authorization|cookie|password|passwd|secret|token|apikey|privatekey|passphrase|credential|licensekey/.test(name.toLowerCase().replace(/[_ -]/g, ""));
}

export function taskRunDefaults(names: string[], variables: WorkspaceVariableLike[], environment: WorkspaceEnvironmentLike | null) {
  const merged = mergeWorkspaceVariables(variables, environment);
  const defaults = defaultTaskRunInputs(names, merged);
  const environmentKeys = new Set((environment?.variables ?? []).filter((v) => v.isEnabled && !v.deletedAt).map((v) => v.key.trim().toLowerCase()));
  const sources: Record<string, SshTaskInputSource> = {};
  for (const name of names) {
    const key = name.toLowerCase();
    sources[name] = environmentKeys.has(key) ? "environment" : merged.has(key) ? "workspace" : "manual";
  }
  return { ...defaults, sources, secretNames: names.filter((name) => sensitiveTaskInput(name) || defaults.secretNames.includes(name)) };
}

// Only metadata for the explicitly selected sources travels back to execution.
export function taskInputVersions(sources: Record<string, SshTaskInputSource>, variables: WorkspaceVariableLike[], environment: WorkspaceEnvironmentLike | null) {
  const versions: NonNullable<SshTaskRunContextInput["inputVersions"]> = {};
  for (const [name, source] of Object.entries(sources)) {
    if (source === "manual") continue;
    const candidates = source === "workspace" ? variables : environment?.variables ?? [];
    const variable = candidates.find((v) => v.isEnabled && !v.deletedAt && v.key.trim().toLowerCase() === name.trim().toLowerCase());
    if (variable?.id && variable.revision !== undefined) versions[name] = { id: variable.id, revision: variable.revision };
  }
  return versions;
}

export function restoreTaskRunInputs(names: string[], variables: WorkspaceVariableLike[], environment: WorkspaceEnvironmentLike | null, run: SshTaskRun) {
  const defaults = taskRunDefaults(names, variables, environment);
  const workspace = taskRunDefaults(names, variables, null);
  const secretNames = new Set(defaults.secretNames);
  if (run.context?.version !== 1) return defaults;
  for (const name of names) {
    const saved = run.context.inputs[name];
    if (!saved) continue;
    if (saved.secret) secretNames.add(name);
    if (saved.source === "workspace" && workspace.secretNames.includes(name)) secretNames.add(name);
    defaults.sources[name] = saved.source;
    if (saved.source === "manual") {
      defaults.inputs[name] = secretNames.has(name) ? "" : saved.value ?? "";
      if (saved.value === undefined) secretNames.add(name);
    } else if (saved.source === "workspace") {
      defaults.inputs[name] = workspace.inputs[name] ?? "";
    } else if (taskRunDefaults([name], [], environment).sources[name] !== "environment") {
      // A removed environment variable must not fall back to a workspace value.
      defaults.inputs[name] = "";
    }
  }
  defaults.secretNames = [...secretNames];
  defaults.filledFromWorkspace = names.filter((name) => defaults.sources[name] !== "manual" && Boolean(defaults.inputs[name]));
  return defaults;
}

export function taskRunSelection(detail: SshTaskDetail, variables: WorkspaceVariableLike[], environments: WorkspaceEnvironmentLike[], run?: SshTaskRun) {
  const context = run?.context?.version === 1 ? run.context : null;
  const environmentId = context ? context.environmentId ?? "" : activeWorkspaceEnvironmentId(environments);
  const environment = workspaceEnvironmentById(environments, environmentId);
  const names = detectTaskInputs(detail.steps, true);
  const values = run ? restoreTaskRunInputs(names, variables, environment, run) : taskRunDefaults(names, variables, environment);
  return {
    environmentId, values,
    connectionId: run ? run.connectionId ?? "" : preferredTaskConnectionId(detail.localBinding),
    notice: recallNotice(detail, environment, run),
  };
}

function recallNotice(detail: SshTaskDetail, environment: WorkspaceEnvironmentLike | null, run?: SshTaskRun) {
  if (!run) return null;
  if (run.context?.version !== 1) return "ssh.tasks.run.legacyContext";
  if (run.context.taskVersion !== taskVersion(detail)) return "ssh.tasks.run.taskChanged";
  if (run.context.environmentUpdatedAt !== (environment?.updatedAt ?? null)) return "ssh.tasks.run.environmentChanged";
  return "ssh.tasks.run.restoredContext";
}
