import type { WorkspaceVariableInput } from "../../types";
import { mockStore } from "./state";
import { createMockVariable, replaceMockVariables, updateMockVariable } from "./workspace-variable-records";

export function listMockWorkspaceVariables(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  return mockStore.workspaceVariables
    .filter((variable) => variable.workspaceId === workspaceId && !variable.deletedAt)
    .sort((left, right) => left.sortOrder - right.sortOrder);
}

export function replaceMockWorkspaceVariables(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const inputs = (args?.variables ?? []) as WorkspaceVariableInput[];
  const current = mockStore.workspaceVariables.filter(
    (variable) => variable.workspaceId !== workspaceId,
  );
  const now = new Date().toISOString();
  const next = replaceMockVariables(
    inputs,
    mockStore.workspaceVariables.filter((variable) => variable.workspaceId === workspaceId),
    workspaceId,
    now,
  );
  mockStore.workspaceVariables = [...current, ...next];
  return next;
}

export function createMockWorkspaceVariable(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const input = args?.input as WorkspaceVariableInput;
  const now = new Date().toISOString();
  const variable = createMockVariable(input, workspaceId, now);
  mockStore.workspaceVariables.push(variable);
  return variable;
}

export function updateMockWorkspaceVariable(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const variableId = String(args?.variableId ?? "");
  const input = args?.input as WorkspaceVariableInput;
  const variable = mockStore.workspaceVariables.find(
    (item) => item.id === variableId && item.workspaceId === workspaceId,
  );
  if (!variable) throw new Error("workspace variable not found");
  updateMockVariable(variable, input, variableId);
  return variable;
}

export function deleteMockWorkspaceVariable(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const variableId = String(args?.variableId ?? "");
  mockStore.workspaceVariables = mockStore.workspaceVariables.filter(
    (item) => item.workspaceId !== workspaceId || item.id !== variableId,
  );
  return mockStore.workspaceVariables.filter(
    (item) => item.workspaceId === workspaceId,
  );
}

export function resolveMockWorkspaceVariables(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId =
    args?.activeEnvironmentId == null ? null : String(args.activeEnvironmentId);
  const environment = environmentId
    ? mockStore.workspaceEnvironments.find(
        (item) => item.id === environmentId && item.workspaceId === workspaceId,
      )
    : null;
  if (environmentId && !environment) throw new Error("workspace environment not found");
  const values = new Map(
    mockStore.workspaceVariables
      .filter((variable) => variable.workspaceId === workspaceId && variable.isEnabled)
      .map((variable) => [variable.key, variable.value]),
  );
  for (const variable of environment?.variables ?? []) {
    if (variable.isEnabled) values.set(variable.key, variable.value);
  }
  const input = String(args?.input ?? "");
  return input.replace(/\{\{\s*([^{}]+?)\s*\}\}/g, (_token, key: string) => {
    const value = values.get(key);
    if (value === undefined) throw new Error(`unresolved variable: ${key}`);
    return value;
  });
}
