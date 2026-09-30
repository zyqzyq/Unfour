import { QueryClient, QueryObserver } from "@tanstack/react-query";
import { expect, it } from "vitest";
import type { FlowDefinition, WorkspaceEnvironment, WorkspaceVariable } from "@unfour/command-client";
import { getFlowPageValidation } from "./flowPageValidation";
import type { Resources } from "./model";
import type { FlowWorkspaceData } from "./useFlowWorkspaceData";

function query<T>(data?: T) {
  return new QueryObserver<T>(new QueryClient(), { queryKey: ["fixture"], initialData: data, enabled: false }).getCurrentResult();
}
function variable(key: string, value: string, overrides: Partial<WorkspaceVariable> = {}): WorkspaceVariable {
  return { id: key, workspaceId: "ws", key, value, isEnabled: true, isSecret: false, description: null,
    sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null, revision: 1, ...overrides };
}
function environment(id: string, variables: WorkspaceVariable[]): WorkspaceEnvironment {
  return { id, workspaceId: "ws", name: id, sortOrder: 0, isActive: false, createdAt: "", updatedAt: "", deletedAt: null,
    revision: 1, variables: variables.map((item) => ({ ...item, environmentId: id })) };
}
const resources: Resources = {
  api: [{ id: "api", name: "API", url: "{{HOST}}", method: "GET" }], database: [], connections: [{ id: "ssh", name: "SSH" }],
  ssh: [{ id: "task", name: "Task", detail: {
    task: { id: "task", workspaceId: "ws", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null },
    steps: [], localBinding: null, detectedInputs: ["Host"],
  } }],
};
const flow: FlowDefinition = { id: "f", workspaceId: "ws", name: "Flow", revision: 1, inputs: [], steps: [
  { id: "api", name: "API", kind: "action", timeoutMs: 1000, action: { capability: "api", resourceId: "api", arguments: {} } },
  { id: "ssh", name: "SSH", kind: "action", timeoutMs: 1000, action: { capability: "ssh", resourceId: "task", connectionId: "ssh", arguments: { workspaceDefaults: true } } },
] };
function data(workspace: WorkspaceVariable[], environments: WorkspaceEnvironment[]): FlowWorkspaceData {
  return { flows: query([flow]), resources, resourcesQuery: query(resources), workspaceVariables: query(workspace), environments: query(environments) };
}
function validate(draft: FlowDefinition, source: FlowWorkspaceData, environmentId = "selected") {
  return getFlowPageValidation(draft, { invalid: {}, environmentId, inputs: {}, secretInputNames: "" }, source);
}

it("keeps selected SSH overrides, exact API keys and suggestions from all environments", () => {
  const source = data([variable("HOST", "workspace"), variable("DISABLED", "", { isEnabled: false })], [
    environment("selected", [variable(" host ", ""), variable("DELETED", "", { deletedAt: "deleted" })]),
    environment("other", [variable("OTHER", "other")]),
  ]);
  const result = validate(flow, source);
  expect(result.environmentKeys).toEqual(["HOST", " host ", "OTHER"]);
  expect(result.apiProblems).toEqual([]);
  expect(result.environmentInputProblems).toEqual([{ stepId: "ssh", name: "SSH", key: "flow.sshMissingInputs" }]);
  expect(result.invalidRun).toBe(true);
  source.environments = query([environment("selected", [variable(" host ", "environment")])]);
  expect(validate(flow, source).invalidRun).toBe(false);
  source.workspaceVariables = query([variable("host", "workspace")]);
  expect(validate(flow, source).apiProblems).toEqual([{ stepId: "api", name: "API", variable: "HOST" }]);
});

it("ignores disabled/deleted overrides and keeps explicit SSH input validation", () => {
  const source = data([variable("HOST", "workspace")], [environment("selected", [
    variable("host", "", { isEnabled: false }), variable("Host", "", { deletedAt: "deleted" }),
  ])]);
  expect(validate(flow, source).invalidRun).toBe(false);
  const explicit: FlowDefinition = { ...flow, steps: [{ ...flow.steps[1], kind: "action", action: {
    capability: "ssh", resourceId: "task", connectionId: "ssh", arguments: { workspaceDefaults: true, inputs: { Host: "" } },
  } }] };
  expect(validate(explicit, source).resourceProblems).toContainEqual({ stepId: "ssh", name: "SSH", key: "flow.sshMissingInputs" });
  expect(validate(explicit, source).invalidRun).toBe(true);
});

it("defers resource checks while defaults load and rejects a missing selected environment", () => {
  const source = data([], [environment("selected", [])]);
  const pending = { ...source, workspaceVariables: query<WorkspaceVariable[]>() };
  expect(validate(flow, pending)).toMatchObject({ hasApi: true, needsWorkspaceDefaults: true, apiProblems: [], environmentInputProblems: [], invalidEnvironment: true });
  const wait: FlowDefinition = { ...flow, steps: [{ id: "wait", name: "Wait", kind: "wait", durationMs: 1, timeoutMs: 1000 }] };
  expect(validate(wait, pending).invalidEnvironment).toBe(false);
  expect(validate(wait, source, "missing").invalidEnvironment).toBe(true);
  expect(validate(wait, source, "").invalidEnvironment).toBe(false);
});

it.each(["poll", "waitUntil"] as const)("includes API %s probes in environment requirements", (kind) => {
  const probe = { capability: "api" as const, resourceId: "api", arguments: {} };
  const predicate = { left: true, op: "eq" as const, right: true };
  const draft: FlowDefinition = { ...flow, steps: [kind === "poll"
    ? { id: "probe", name: "Probe", kind, timeoutMs: 1000, probe, predicate, intervalMs: 10, maxAttempts: 2 }
    : { id: "probe", name: "Probe", kind, timeoutMs: 1000, probe, successWhen: predicate, intervalMs: 10 }] };
  expect(validate(draft, data([], [environment("selected", [])]))).toMatchObject({ hasApi: true, apiProblems: [{ stepId: "probe", name: "Probe", variable: "HOST" }], invalidRun: true });
});
