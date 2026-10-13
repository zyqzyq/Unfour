import * as lifecycle from "./workspace-lifecycle";
import * as variables from "./workspace-variables";
import * as environments from "./workspace-environments";
import { UNHANDLED, type MockResult } from "./types";

// Keep command registration separate from lifecycle and variable semantics.
// Presence, not the returned value, determines whether a command is handled.
const handlers = new Map<string, (args?: Record<string, unknown>) => unknown>([
  ["workspace_bundle_pick", () => null],
  ["workspace_bundle_export", () => ({ saved: false })],
  ["workspace_environment_import_preview", () => null],
  ["workspace_environment_export", () => ({ saved: false })],
  ["workspace_list", lifecycle.listMockWorkspaces],
  ["workspace_create", lifecycle.createMockWorkspace],
  ["workspace_update_environment", lifecycle.updateMockWorkspaceEnvironment],
  ["workspace_update_mcp_policy", lifecycle.updateMockWorkspacePolicy],
  ["workspace_set_default", lifecycle.setMockDefaultWorkspace],
  ["workspace_rename", lifecycle.renameMockWorkspace],
  ["workspace_delete", lifecycle.deleteMockWorkspace],
  ["workspace_set_active", lifecycle.setMockActiveWorkspace],
  ["workspace_layout_get", lifecycle.getWorkspaceMockLayout],
  ["workspace_layout_update", lifecycle.updateWorkspaceMockLayout],
  ["workspace_variables_list", variables.listMockWorkspaceVariables],
  ["workspace_variables_replace", variables.replaceMockWorkspaceVariables],
  ["workspace_variable_create", variables.createMockWorkspaceVariable],
  ["workspace_variable_update", variables.updateMockWorkspaceVariable],
  ["workspace_variable_delete", variables.deleteMockWorkspaceVariable],
  ["workspace_variables_resolve", variables.resolveMockWorkspaceVariables],
  ["workspace_environments_list", environments.listMockWorkspaceEnvironments],
  ["workspace_environment_create", environments.createMockWorkspaceEnvironment],
  ["workspace_environment_update", environments.updateMockWorkspaceEnvironment],
  ["workspace_environment_update_metadata", environments.updateMockEnvironmentMetadata],
  ["workspace_environments_reorder", environments.reorderMockEnvironments],
  ["workspace_environment_delete", environments.deleteMockEnvironment],
  ["workspace_environment_set_active", environments.setMockActiveEnvironment],
  ["workspace_environment_variable_create", environments.createMockEnvironmentVariable],
  ["workspace_environment_variable_update", environments.updateMockEnvironmentVariable],
  ["workspace_environment_variables_replace", environments.replaceMockEnvironmentVariables],
  ["workspace_environment_variable_delete", environments.deleteMockEnvironmentVariable],
]);

export function handleWorkspaceMock<T>(command: string, args?: Record<string, unknown>): MockResult<T> {
  const handler = handlers.get(command);
  return handler ? handler(args) as T : UNHANDLED;
}
