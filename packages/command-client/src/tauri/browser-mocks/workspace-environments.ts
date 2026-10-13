import type { WorkspaceEnvironment, WorkspaceEnvironmentVariable, WorkspaceVariableInput } from "../../types";
import { mockStore } from "./state";
import { createMockVariable, replaceMockVariables, updateMockVariable } from "./workspace-variable-records";

export function listMockWorkspaceEnvironments(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  return workspaceEnvironmentList(workspaceId);
}

export function createMockWorkspaceEnvironment(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const now = new Date().toISOString();
  const existing = workspaceEnvironmentList(workspaceId);
  const environment: WorkspaceEnvironment = {
    id: crypto.randomUUID(),
    workspaceId,
    name: String(args?.name ?? "New Environment"),
    sortOrder: existing.length,
    // Mirror backend: the first environment becomes active.
    isActive: existing.length === 0,
    variables: [],
    createdAt: now,
    updatedAt: now,
    deletedAt: null,
    revision: 1,
  };
  mockStore.workspaceEnvironments.push(environment);
  return environment;
}

export function updateMockWorkspaceEnvironment(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  const inputs = (args?.variables ?? []) as WorkspaceVariableInput[];
  const now = new Date().toISOString();
  environment.name = String(args?.name ?? environment.name);
  environment.updatedAt = now;
  environment.revision += 1;
  replaceEnvironmentVariableRecords(environment, inputs, now);
  return environment;
}

export function updateMockEnvironmentMetadata(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  environment.name = String(args?.name ?? environment.name);
  environment.sortOrder = Number(args?.sortOrder ?? environment.sortOrder);
  environment.updatedAt = new Date().toISOString();
  environment.revision += 1;
  return environment;
}

export function reorderMockEnvironments(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentIds = (args?.environmentIds ?? []) as string[];
  const activeIds = mockStore.workspaceEnvironments
    .filter((environment) => environment.workspaceId === workspaceId && !environment.deletedAt)
    .map((environment) => environment.id);
  if (
    environmentIds.length !== activeIds.length ||
    new Set(environmentIds).size !== environmentIds.length ||
    environmentIds.some((id) => !activeIds.includes(id))
  ) {
    throw new Error("environment reorder must include every active environment exactly once");
  }
  environmentIds.forEach((id, sortOrder) => {
    const environment = findWorkspaceEnvironment(workspaceId, id);
    if (environment.sortOrder !== sortOrder) {
      environment.sortOrder = sortOrder;
      environment.updatedAt = new Date().toISOString();
      environment.revision += 1;
    }
  });
  return workspaceEnvironmentList(workspaceId);
}

export function deleteMockEnvironment(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  const wasActive = environment.isActive;
  mockStore.workspaceEnvironments = mockStore.workspaceEnvironments.filter(
    (item) => item !== environment,
  );
  const remaining = workspaceEnvironmentList(workspaceId);
  if (wasActive && remaining[0]) remaining[0].isActive = true;
  return remaining;
}

export function setMockActiveEnvironment(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = args?.environmentId == null ? null : String(args.environmentId);
  if (
    environmentId &&
    !mockStore.workspaceEnvironments.some(
      (environment) =>
        environment.id === environmentId && environment.workspaceId === workspaceId,
    )
  ) {
    throw new Error("workspace environment not found");
  }
  for (const environment of mockStore.workspaceEnvironments) {
    if (environment.workspaceId === workspaceId) {
      environment.isActive = environment.id === environmentId;
    }
  }
  return workspaceEnvironmentList(workspaceId);
}

export function createMockEnvironmentVariable(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const input = args?.input as WorkspaceVariableInput;
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  const now = new Date().toISOString();
  const variable: WorkspaceEnvironmentVariable = {
    ...createMockVariable(input, workspaceId, now),
    environmentId,
  };
  environment.variables.push(variable);
  return variable;
}

export function updateMockEnvironmentVariable(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const variableId = String(args?.variableId ?? "");
  const input = args?.input as WorkspaceVariableInput;
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  const variable = environment.variables.find((item) => item.id === variableId);
  if (!variable) throw new Error("workspace environment variable not found");
  updateMockVariable(variable, input, variableId);
  return variable;
}

export function replaceMockEnvironmentVariables(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const inputs = (args?.variables ?? []) as WorkspaceVariableInput[];
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  const now = new Date().toISOString();
  replaceEnvironmentVariableRecords(environment, inputs, now);
  return environment.variables;
}

export function deleteMockEnvironmentVariable(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const environmentId = String(args?.environmentId ?? "");
  const variableId = String(args?.variableId ?? "");
  const environment = findWorkspaceEnvironment(workspaceId, environmentId);
  environment.variables = environment.variables.filter((item) => item.id !== variableId);
  return environment.variables;
}

function workspaceEnvironmentList(workspaceId: string) {
  return mockStore.workspaceEnvironments
    .filter((environment) => environment.workspaceId === workspaceId && !environment.deletedAt)
    .sort((left, right) => left.sortOrder - right.sortOrder);
}

function findWorkspaceEnvironment(workspaceId: string, environmentId: string) {
  const environment = mockStore.workspaceEnvironments.find(
    (item) => item.id === environmentId && item.workspaceId === workspaceId,
  );
  if (!environment) throw new Error("workspace environment not found");
  return environment;
}

function replaceEnvironmentVariableRecords(environment: WorkspaceEnvironment, inputs: WorkspaceVariableInput[], now: string) {
  environment.variables = replaceMockVariables(inputs, environment.variables, environment.workspaceId, now)
    .map((variable) => ({ ...variable, environmentId: environment.id }));
}
