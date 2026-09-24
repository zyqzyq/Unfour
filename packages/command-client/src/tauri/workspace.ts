import { call } from "./invoke";

export type EnvironmentImportPreview = {
  format: "unfour" | "postman";
  name: string;
  variables: { key: string; isSecret: boolean; isEnabled: boolean }[];
  conflict: boolean;
  warnings: string[];
};

export function previewEnvironmentImport(workspaceId: string) {
  return call<{content: string; preview: EnvironmentImportPreview} | null>("workspace_environment_import_preview", {workspaceId});
}

export function importEnvironment(workspaceId: string, content: string) {
  return call<WorkspaceEnvironment>("workspace_environment_import", {workspaceId, content});
}

export function exportEnvironment(workspaceId: string, environmentId: string, format: "unfour" | "postman") {
  return call<{saved: boolean}>("workspace_environment_export", {workspaceId, environmentId, format});
}
import type {
  Workspace,
  WorkspaceEnvironment,
  WorkspaceEnvironmentVariable,
  WorkspaceEnvironmentType,
  WorkspaceLayout,
  WorkspaceMcpPolicy,
  WorkspaceState,
  WorkspaceVariable,
  WorkspaceVariableInput,
} from "../types";

export function getWorkspaceState() {
  return call<WorkspaceState>("workspace_list");
}

export function createWorkspace(
  name: string,
  environmentType?: WorkspaceEnvironmentType,
  mcpPolicy?: WorkspaceMcpPolicy,
) {
  return call<Workspace>("workspace_create", { name, environmentType, mcpPolicy });
}

export function renameWorkspace(workspaceId: string, name: string) {
  return call<Workspace>("workspace_rename", { workspaceId, name });
}

export function deleteWorkspace(workspaceId: string) {
  return call<WorkspaceState>("workspace_delete", { workspaceId });
}

export function setActiveWorkspace(workspaceId: string) {
  return call<WorkspaceState>("workspace_set_active", { workspaceId });
}

export function updateWorkspaceEnvironment(
  workspaceId: string,
  environmentType: WorkspaceEnvironmentType,
) {
  return call<Workspace>("workspace_update_environment", {
    workspaceId,
    environmentType,
  });
}

export function updateWorkspaceMcpPolicy(
  workspaceId: string,
  mcpPolicy: WorkspaceMcpPolicy,
) {
  return call<Workspace>("workspace_update_mcp_policy", { workspaceId, mcpPolicy });
}

export function setDefaultWorkspace(workspaceId: string) {
  return call<WorkspaceState>("workspace_set_default", { workspaceId });
}

export function getWorkspaceLayout(workspaceId: string) {
  return call<WorkspaceLayout>("workspace_layout_get", { workspaceId });
}

export function updateWorkspaceLayout(workspaceId: string, layout: WorkspaceLayout) {
  return call<WorkspaceLayout>("workspace_layout_update", {
    workspaceId,
    layout,
  });
}

export function listWorkspaceVariables(workspaceId: string) {
  return call<WorkspaceVariable[]>("workspace_variables_list", { workspaceId });
}

export function replaceWorkspaceVariables(
  workspaceId: string,
  variables: WorkspaceVariableInput[],
) {
  return call<WorkspaceVariable[]>("workspace_variables_replace", {
    workspaceId,
    variables,
  });
}

export function createWorkspaceVariable(
  workspaceId: string,
  input: WorkspaceVariableInput,
) {
  return call<WorkspaceVariable>("workspace_variable_create", { workspaceId, input });
}

export function updateWorkspaceVariable(
  workspaceId: string,
  variableId: string,
  input: WorkspaceVariableInput,
) {
  return call<WorkspaceVariable>("workspace_variable_update", {
    workspaceId,
    variableId,
    input,
  });
}

export function deleteWorkspaceVariable(workspaceId: string, variableId: string) {
  return call<WorkspaceVariable[]>("workspace_variable_delete", {
    workspaceId,
    variableId,
  });
}

export function listWorkspaceEnvironments(workspaceId: string) {
  return call<WorkspaceEnvironment[]>("workspace_environments_list", { workspaceId });
}

export function createWorkspaceEnvironment(workspaceId: string, name: string) {
  return call<WorkspaceEnvironment>("workspace_environment_create", {
    workspaceId,
    name,
  });
}

export function updateWorkspaceEnvironmentVariables(
  workspaceId: string,
  environmentId: string,
  name: string,
  variables: WorkspaceVariableInput[],
) {
  return call<WorkspaceEnvironment>("workspace_environment_update", {
    workspaceId,
    environmentId,
    name,
    variables,
  });
}

export function updateWorkspaceEnvironmentMetadata(
  workspaceId: string,
  environmentId: string,
  name: string,
  sortOrder: number,
) {
  return call<WorkspaceEnvironment>("workspace_environment_update_metadata", {
    workspaceId,
    environmentId,
    name,
    sortOrder,
  });
}

export function reorderWorkspaceEnvironments(
  workspaceId: string,
  environmentIds: string[],
) {
  return call<WorkspaceEnvironment[]>("workspace_environments_reorder", {
    workspaceId,
    environmentIds,
  });
}

export function deleteWorkspaceEnvironment(
  workspaceId: string,
  environmentId: string,
) {
  return call<WorkspaceEnvironment[]>("workspace_environment_delete", {
    workspaceId,
    environmentId,
  });
}

export function setActiveWorkspaceEnvironment(
  workspaceId: string,
  environmentId: string | null,
) {
  return call<WorkspaceEnvironment[]>("workspace_environment_set_active", {
    workspaceId,
    environmentId,
  });
}

export function createWorkspaceEnvironmentVariable(
  workspaceId: string,
  environmentId: string,
  input: WorkspaceVariableInput,
) {
  return call<WorkspaceEnvironmentVariable>("workspace_environment_variable_create", {
    workspaceId,
    environmentId,
    input,
  });
}

export function updateWorkspaceEnvironmentVariable(
  workspaceId: string,
  environmentId: string,
  variableId: string,
  input: WorkspaceVariableInput,
) {
  return call<WorkspaceEnvironmentVariable>("workspace_environment_variable_update", {
    workspaceId,
    environmentId,
    variableId,
    input,
  });
}

export function replaceWorkspaceEnvironmentVariables(
  workspaceId: string,
  environmentId: string,
  variables: WorkspaceVariableInput[],
) {
  return call<WorkspaceEnvironmentVariable[]>("workspace_environment_variables_replace", {
    workspaceId,
    environmentId,
    variables,
  });
}

export function deleteWorkspaceEnvironmentVariable(
  workspaceId: string,
  environmentId: string,
  variableId: string,
) {
  return call<WorkspaceEnvironmentVariable[]>("workspace_environment_variable_delete", {
    workspaceId,
    environmentId,
    variableId,
  });
}

export function resolveWorkspaceVariables(
  workspaceId: string,
  activeEnvironmentId: string | null,
  input: string,
) {
  return call<string>("workspace_variables_resolve", {
    workspaceId,
    activeEnvironmentId,
    input,
  });
}
