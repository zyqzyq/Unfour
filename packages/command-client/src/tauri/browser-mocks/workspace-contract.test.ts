import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Workspace, WorkspaceEnvironment, WorkspaceLayout, WorkspaceVariable, WorkspaceVariableInput } from "../../types";
import { mockInvoke } from "./index";
import { mockState, mockStore, mockWorkspace } from "./state";
import { UNHANDLED } from "./types";
import { handleWorkspaceMock } from "./workspace";

const initial = structuredClone({ state: mockState, variables: mockStore.workspaceVariables,
  environments: mockStore.workspaceEnvironments, layouts: mockStore.layouts });
const input: WorkspaceVariableInput = {
  key: "HOST", value: "fixture", isSecret: false, isEnabled: true, description: null, sortOrder: 9,
};
const createdAt = "2026-01-01T00:00:00.000Z";
const updatedAt = "2026-01-02T00:00:00.000Z";

beforeEach(() => {
  mockState.workspaces = [
    { ...mockWorkspace, id: "w1", isDefault: true, revision: 1 },
    { ...mockWorkspace, id: "w2", isDefault: false, revision: 1 },
  ];
  mockState.activeWorkspaceId = "w1";
  mockStore.workspaceVariables = [];
  mockStore.workspaceEnvironments = [];
  mockStore.layouts = {};
  vi.useFakeTimers();
  vi.setSystemTime(new Date(createdAt));
});

afterEach(() => {
  const restored = structuredClone(initial);
  Object.assign(mockState, restored.state);
  mockStore.workspaceVariables = restored.variables;
  mockStore.workspaceEnvironments = restored.environments;
  mockStore.layouts = restored.layouts;
  vi.useRealTimers();
});

function createEnvironment(workspaceId = "w1", name = "Environment") {
  return mockInvoke<WorkspaceEnvironment>("workspace_environment_create", { workspaceId, name });
}

describe("workspace command routing and lifecycle", () => {
  it("preserves unsupported browser picker/export results and unknown command fallthrough", async () => {
    for (const command of ["workspace_bundle_pick", "workspace_environment_import_preview"]) {
      await expect(mockInvoke(command)).resolves.toBeNull();
    }
    for (const command of ["workspace_bundle_export", "workspace_environment_export"]) {
      await expect(mockInvoke(command)).resolves.toEqual({ saved: false });
    }
    expect(handleWorkspaceMock("unknown")).toBe(UNHANDLED);
    expect(handleWorkspaceMock("toString")).toBe(UNHANDLED);
    await expect(mockInvoke("workspace_unknown")).rejects.toThrow("Mock command is not implemented");
  });

  it("preserves creation defaults, active selection and differing metadata revision rules", async () => {
    const workspace = await mockInvoke<Workspace>("workspace_create");
    expect(workspace).toMatchObject({ name: "New Workspace", environmentType: "dev", mcpPolicy: "auto", isDefault: false });
    expect(mockState.activeWorkspaceId).toBe(workspace.id);
    expect(mockState.workspaces[0]).toBe(workspace);
    const revision = workspace.revision;
    vi.setSystemTime(new Date(updatedAt));
    await mockInvoke("workspace_rename", { workspaceId: workspace.id, name: "Renamed" });
    await mockInvoke("workspace_update_environment", { workspaceId: workspace.id, environmentType: "test" });
    expect(workspace).toMatchObject({ name: "Renamed", environmentType: "test", revision, updatedAt });
    await mockInvoke("workspace_update_mcp_policy", { workspaceId: workspace.id, mcpPolicy: "guarded" });
    expect(workspace).toMatchObject({ mcpPolicy: "guarded", revision: revision + 1 });
    await expect(mockInvoke("workspace_rename", { workspaceId: "missing" })).rejects.toThrow("workspace not found");
  });

  it("changes only affected default records and preserves the default on repeated selection", async () => {
    await mockInvoke("workspace_set_default", { workspaceId: "w2" });
    expect(mockState.workspaces.map((w) => [w.id, w.isDefault, w.revision])).toEqual([["w1", false, 2], ["w2", true, 2]]);
    vi.setSystemTime(new Date(updatedAt));
    await mockInvoke("workspace_set_default", { workspaceId: "w2" });
    expect(mockState.workspaces.map((w) => w.updatedAt)).toEqual([createdAt, createdAt]);
    await expect(mockInvoke("workspace_set_default", { workspaceId: "missing" })).rejects.toThrow("workspace not found");
    await mockInvoke("workspace_delete", { workspaceId: "w1" });
    expect(mockState.activeWorkspaceId).toBe("w2");
    await expect(mockInvoke("workspace_delete", { workspaceId: "w2" })).rejects.toThrow("at least one workspace must remain");
  });

  it("isolates layouts and keeps the active-workspace fallback", async () => {
    const layout = await mockInvoke<WorkspaceLayout>("workspace_layout_get");
    expect(layout.workspaceId).toBe("w1");
    await mockInvoke("workspace_layout_update", { layout: { ...layout, workspaceId: "ignored", sidebarCollapsed: true } });
    await expect(mockInvoke("workspace_layout_get")).resolves.toMatchObject({ workspaceId: "w1", sidebarCollapsed: true, updatedAt: createdAt });
    await mockInvoke("workspace_set_active", { workspaceId: "w2" });
    await expect(mockInvoke("workspace_layout_get")).resolves.toMatchObject({ workspaceId: "w2", sidebarCollapsed: false });
    await expect(mockInvoke("workspace_list")).resolves.toBe(mockState);
  });
});

describe("workspace variable record contracts", () => {
  it.each(["workspace_variables_replace", "workspace_environment_update", "workspace_environment_variables_replace"])(
    "%s retains scoped identity, creation time, revision and caller order", async (command) => {
      const environment = await createEnvironment();
      const existing: WorkspaceVariable = { ...input, id: "existing", workspaceId: "w1", createdAt,
        updatedAt: createdAt, deletedAt: null, revision: 4 };
      const foreign = { ...existing, workspaceId: "w2", value: "foreign" };
      mockStore.workspaceVariables = [foreign, existing];
      environment.variables = [{ ...existing, environmentId: environment.id }];
      vi.setSystemTime(new Date(updatedAt));
      const variables = [{ ...input, id: "new-id", key: "NEW" }, { ...input, id: existing.id, value: "changed", isSecret: true }];
      const result = await mockInvoke<WorkspaceVariable[] | WorkspaceEnvironment>(command, {
        workspaceId: "w1", environmentId: environment.id, name: "Renamed", variables,
      });
      const records = Array.isArray(result) ? result : result.variables;
      expect(records.map((v) => [v.key, v.sortOrder])).toEqual([["NEW", 0], ["HOST", 1]]);
      expect(records[0].id).not.toBe("new-id");
      expect(records[0]).toMatchObject({ workspaceId: "w1", createdAt: updatedAt, updatedAt, revision: 1 });
      expect(records[1]).toMatchObject({ id: "existing", workspaceId: "w1", value: "changed", isSecret: true,
        createdAt, updatedAt, revision: 5 });
      expect(foreign.value).toBe("foreign");
      expect(mockStore.workspaceVariables).toContain(foreign);
      if (command !== "workspace_variables_replace") {
        expect(environment.variables.every((v) => v.environmentId === environment.id)).toBe(true);
        expect(environment.revision).toBe(command === "workspace_environment_update" ? 2 : 1);
      }
    },
  );

  it.each([false, true])("preserves CRUD object identity and scope (environment=%s)", async (inEnvironment) => {
    const environment = await createEnvironment();
    const scope = { workspaceId: "w1", environmentId: environment.id };
    const prefix = inEnvironment ? "workspace_environment_variable" : "workspace_variable";
    const record = await mockInvoke<WorkspaceVariable>(`${prefix}_create`, { ...scope, input: { ...input, id: "ignored" } });
    expect(record.id).not.toBe("ignored");
    expect(record.sortOrder).toBe(9);
    vi.setSystemTime(new Date(updatedAt));
    const changed = await mockInvoke<WorkspaceVariable>(`${prefix}_update`, {
      ...scope, variableId: record.id, input: { ...input, id: "ignored", value: "new" },
    });
    expect(changed).toBe(record);
    expect(changed).toMatchObject({ value: "new", createdAt, updatedAt, revision: 2 });
    await expect(mockInvoke(`${prefix}_update`, { ...scope, workspaceId: "w2", variableId: record.id, input }))
      .rejects.toThrow(inEnvironment ? "workspace environment not found" : "workspace variable not found");
    await expect(mockInvoke(`${prefix}_delete`, { ...scope, variableId: record.id })).resolves.toEqual([]);
  });

  it("lists live workspace variables in sort order without sorting the backing array", async () => {
    const make = (id: string, sortOrder: number, deletedAt: string | null = null): WorkspaceVariable =>
      ({ ...input, id, sortOrder, workspaceId: "w1", createdAt, updatedAt: createdAt, deletedAt, revision: 1 });
    const records = [make("late", 2), make("deleted", 0, createdAt), make("early", 1)];
    mockStore.workspaceVariables = records;
    await expect(mockInvoke("workspace_variables_list", { workspaceId: "w1" })).resolves.toEqual([records[2], records[0]]);
    expect(mockStore.workspaceVariables.map((v) => v.id)).toEqual(["late", "deleted", "early"]);
  });
});

describe("workspace environments and resolution", () => {
  it("activates the first environment, supports no active selection and selects the first remaining on delete", async () => {
    const first = await createEnvironment("w1", "First");
    const second = await createEnvironment("w1", "Second");
    const other = await createEnvironment("w2");
    expect([first.isActive, second.isActive, other.isActive]).toEqual([true, false, true]);
    await expect(mockInvoke("workspace_environment_set_active", { workspaceId: "w1", environmentId: other.id }))
      .rejects.toThrow("workspace environment not found");
    await mockInvoke("workspace_environment_set_active", { workspaceId: "w1", environmentId: null });
    expect([first.isActive, second.isActive, other.isActive]).toEqual([false, false, true]);
    await mockInvoke("workspace_environment_set_active", { workspaceId: "w1", environmentId: second.id });
    await expect(mockInvoke("workspace_environment_delete", { workspaceId: "w1", environmentId: second.id })).resolves.toEqual([first]);
    expect(first.isActive).toBe(true);
    expect([first.revision, other.revision]).toEqual([1, 1]);
    await expect(mockInvoke("workspace_environment_delete", { workspaceId: "w1", environmentId: first.id })).resolves.toEqual([]);
  });

  it("validates reordering before mutation and only updates changed positions", async () => {
    const first = await createEnvironment("w1", "First");
    const second = await createEnvironment("w1", "Second");
    const other = await createEnvironment("w2");
    for (const environmentIds of [[], [first.id, first.id], [first.id, other.id]]) {
      await expect(mockInvoke("workspace_environments_reorder", { workspaceId: "w1", environmentIds }))
        .rejects.toThrow("every active environment exactly once");
    }
    expect([first.sortOrder, second.sortOrder, first.revision, second.revision]).toEqual([0, 1, 1, 1]);
    vi.setSystemTime(new Date(updatedAt));
    const args = { workspaceId: "w1", environmentIds: [second.id, first.id] };
    await expect(mockInvoke("workspace_environments_reorder", args)).resolves.toEqual([second, first]);
    await mockInvoke("workspace_environments_reorder", args);
    expect([first.revision, second.revision, other.revision]).toEqual([2, 2, 1]);
    await mockInvoke("workspace_environment_update_metadata", { workspaceId: "w1", environmentId: first.id, name: "Renamed", sortOrder: 3 });
    expect(first).toMatchObject({ name: "Renamed", sortOrder: 3, revision: 3, variables: [] });
  });

  it("resolves exact keys with enabled environment overrides and validates explicit environment scope", async () => {
    const environment = await createEnvironment();
    await mockInvoke("workspace_variables_replace", { workspaceId: "w1", variables: [input, { ...input, key: "EMPTY", value: "" }] });
    await mockInvoke("workspace_environment_variables_replace", { workspaceId: "w1", environmentId: environment.id,
      variables: [{ ...input, value: "environment" }, { ...input, key: "EMPTY", value: "disabled", isEnabled: false }] });
    const args = { workspaceId: "w1", activeEnvironmentId: environment.id, input: "{{ HOST }}:{{EMPTY}}" };
    await expect(mockInvoke("workspace_variables_resolve", args)).resolves.toBe("environment:");
    await expect(mockInvoke("workspace_variables_resolve", { ...args, activeEnvironmentId: null })).resolves.toBe("fixture:");
    await expect(mockInvoke("workspace_variables_resolve", { ...args, input: "{{host}}" })).rejects.toThrow("unresolved variable: host");
    await expect(mockInvoke("workspace_variables_resolve", { ...args, workspaceId: "w2", input: "literal" }))
      .rejects.toThrow("workspace environment not found");
  });
});
