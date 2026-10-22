export type WorkspaceVariableLike = {
  id?: string;
  revision?: number;
  key: string;
  value: string;
  isEnabled: boolean;
  isSecret: boolean;
  deletedAt?: string | null;
};

export type WorkspaceEnvironmentLike = {
  id?: string;
  name?: string;
  isActive: boolean;
  updatedAt?: string;
  revision?: number;
  deletedAt?: string | null;
  variables: WorkspaceVariableLike[];
};

export type MergedWorkspaceVariable = {
  key: string;
  value: string;
  isSecret: boolean;
};

function lookupKey(key: string) {
  return key.trim().toLowerCase();
}

/** Merge workspace-level vars with one explicitly selected environment (env overrides). */
export function mergeWorkspaceVariables(
  workspaceVariables: WorkspaceVariableLike[],
  environment: WorkspaceEnvironmentLike | null,
): Map<string, MergedWorkspaceVariable> {
  const values = new Map<string, MergedWorkspaceVariable>();

  for (const variable of workspaceVariables) {
    if (!variable.isEnabled || variable.deletedAt) continue;
    const key = variable.key.trim();
    if (!key) continue;
    values.set(lookupKey(key), {
      key,
      value: variable.value,
      isSecret: variable.isSecret,
    });
  }

  for (const variable of environment?.variables ?? []) {
    if (!variable.isEnabled || variable.deletedAt) continue;
    const key = variable.key.trim();
    if (!key) continue;
    values.set(lookupKey(key), {
      key,
      value: variable.value,
      isSecret: variable.isSecret,
    });
  }

  return values;
}

export function defaultTaskRunInputs(
  detectedNames: string[],
  variables: Map<string, MergedWorkspaceVariable>,
): {
  inputs: Record<string, string>;
  secretNames: string[];
  filledFromWorkspace: string[];
} {
  const inputs: Record<string, string> = {};
  const secretNames: string[] = [];
  const filledFromWorkspace: string[] = [];

  for (const name of detectedNames) {
    const match = variables.get(lookupKey(name));
    if (!match) {
      inputs[name] = "";
      continue;
    }
    inputs[name] = match.value;
    filledFromWorkspace.push(name);
    if (match.isSecret) secretNames.push(name);
  }

  return { inputs, secretNames, filledFromWorkspace };
}

export function activeWorkspaceEnvironmentName(
  environments: WorkspaceEnvironmentLike[],
): string | null {
  const active = environments.find(
    (environment) => environment.isActive && !environment.deletedAt,
  );
  const name = active?.name?.trim();
  return name || null;
}

export function activeWorkspaceEnvironmentId(
  environments: WorkspaceEnvironmentLike[],
): string {
  return (
    environments.find(
      (environment) => environment.isActive && !environment.deletedAt,
    )?.id ?? ""
  );
}

export function workspaceEnvironmentById(
  environments: WorkspaceEnvironmentLike[],
  environmentId: string,
): WorkspaceEnvironmentLike | null {
  if (!environmentId) return null;
  return (
    environments.find(
      (environment) =>
        environment.id === environmentId && !environment.deletedAt,
    ) ?? null
  );
}
