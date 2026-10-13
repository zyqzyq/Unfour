import type { Workspace, WorkspaceEnvironmentType, WorkspaceLayout, WorkspaceMcpPolicy } from "../../types";
import { getMockLayout, mockState, mockStore, mockWorkspace } from "./state";

export function listMockWorkspaces() {
  return mockState;
}

export function createMockWorkspace(args?: Record<string, unknown>) {
  const environmentType = String(args?.environmentType ?? "dev") as WorkspaceEnvironmentType;
  const mcpPolicy = String(args?.mcpPolicy ?? "auto") as WorkspaceMcpPolicy;
  const workspace: Workspace = {
    ...mockWorkspace,
    id: crypto.randomUUID(),
    name: String(args?.name ?? "New Workspace"),
    isDefault: false,
    environmentType,
    mcpPolicy,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
  };
  mockState.workspaces = [workspace, ...mockState.workspaces];
  mockState.activeWorkspaceId = workspace.id;
  return workspace;
}

export function updateMockWorkspaceEnvironment(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const workspace = mockState.workspaces.find((item) => item.id === workspaceId);
  if (!workspace) throw new Error("workspace not found");
  workspace.environmentType = String(
    args?.environmentType ?? workspace.environmentType,
  ) as WorkspaceEnvironmentType;
  workspace.updatedAt = new Date().toISOString();
  return workspace;
}

export function updateMockWorkspacePolicy(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const workspace = mockState.workspaces.find((item) => item.id === workspaceId);
  if (!workspace) throw new Error("workspace not found");
  workspace.mcpPolicy = String(args?.mcpPolicy ?? workspace.mcpPolicy) as WorkspaceMcpPolicy;
  workspace.updatedAt = new Date().toISOString();
  workspace.revision += 1;
  return workspace;
}

export function setMockDefaultWorkspace(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  if (!mockState.workspaces.some((item) => item.id === workspaceId)) {
    throw new Error("workspace not found");
  }
  const now = new Date().toISOString();
  for (const workspace of mockState.workspaces) {
    const isDefault = workspace.id === workspaceId;
    if (workspace.isDefault !== isDefault) {
      workspace.isDefault = isDefault;
      workspace.updatedAt = now;
      workspace.revision += 1;
    }
  }
  return mockState;
}

export function renameMockWorkspace(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  const workspace = mockState.workspaces.find((item) => item.id === workspaceId);
  if (!workspace) throw new Error("workspace not found");
  workspace.name = String(args?.name ?? workspace.name);
  workspace.updatedAt = new Date().toISOString();
  return workspace;
}

export function deleteMockWorkspace(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? "");
  if (mockState.workspaces.length <= 1) {
    throw new Error("at least one workspace must remain");
  }
  mockState.workspaces = mockState.workspaces.filter((item) => item.id !== workspaceId);
  if (mockState.activeWorkspaceId === workspaceId) {
    mockState.activeWorkspaceId = mockState.workspaces[0]?.id ?? mockWorkspace.id;
  }
  return mockState;
}

export function setMockActiveWorkspace(args?: Record<string, unknown>) {
  mockState.activeWorkspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  return mockState;
}

export function getWorkspaceMockLayout(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  return getMockLayout(workspaceId);
}

export function updateWorkspaceMockLayout(args?: Record<string, unknown>) {
  const workspaceId = String(args?.workspaceId ?? mockState.activeWorkspaceId);
  const layout = args?.layout as WorkspaceLayout;
  mockStore.layouts[workspaceId] = {
    ...layout,
    workspaceId,
    updatedAt: new Date().toISOString(),
  };
  return mockStore.layouts[workspaceId];
}
