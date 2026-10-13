import type { FlowAction, FlowDefinition, SshTaskDetail } from "@unfour/command-client";
import { sqlProblem } from "./actionAuthoring";

export type Resources = {
  api: { id: string; name: string; url?: string; authJson?: string; headersJson?: string; queryJson?: string; body?: string | null; method?: string; bodyKind?: string; preRequestScript?: string | null; postResponseScript?: string | null }[];
  database: { id: string; name: string; readOnly?: boolean }[];
  ssh: { id: string; name: string; detail?: SshTaskDetail }[];
  connections: { id: string; name: string }[];
};

function sshInputsMissing(action: FlowAction, resources: Resources, defaults?: Map<string, string>) {
  const detail = resources.ssh.find((task) => task.id === action.resourceId)?.detail;
  const values = action.arguments.inputs;
  if (!detail || (values && typeof values === "object" && "$ref" in values)) return false;
  const bindings = (values ?? {}) as Record<string, unknown>;
  const useDefaults = action.arguments.workspaceDefaults === true;
  return (detail.detectedInputs ?? []).some((name) => {
    if (Object.prototype.hasOwnProperty.call(bindings, name)) {
      const value = bindings[name];
      return typeof value === "string" && !value.trim();
    }
    if (!useDefaults) return true;
    if (!defaults) return false;
    return !defaults.get(name.toLowerCase())?.trim();
  });
}

/** SSH gaps that appear only after the selected run environment is known. */
export function environmentInputErrors(definition: FlowDefinition, resources: Resources, defaults: Map<string, string>) {
  const saved = new Set(resourceErrors(definition, resources).map((problem) => `${problem.name}\0${problem.key}`));
  return resourceErrors(definition, resources, defaults).filter((problem) => problem.key === "flow.sshMissingInputs" && !saved.has(`${problem.name}\0${problem.key}`));
}

export function resourceErrors(definition: FlowDefinition, resources: Resources, defaults?: Map<string, string>) {
  return definition.steps.flatMap((step) => {
    const probe = step.kind === "poll" || step.kind === "waitUntil";
    const action = step.kind === "action" ? step.action : probe ? step.probe : null;
    if (!action) return [];
    // A missing resource is the only error for this step; downstream checks
    // require its metadata. Preserve both step order and capability error order.
    const keys = resources[action.capability].some((resource) => resource.id === action.resourceId)
      ? actionResourceErrors(action, resources, probe, defaults)
      : ["flow.missingResource"];
    return keys.map((key) => ({ stepId: step.id, name: step.name, key }));
  });
}

function actionResourceErrors(action: FlowAction, resources: Resources, probe: boolean, defaults?: Map<string, string>) {
  switch (action.capability) {
    case "api":
      return apiResourceErrors(resources.api.find((request) => request.id === action.resourceId)!, probe);
    case "database":
      return databaseResourceErrors(action, resources, probe);
    case "ssh":
      return sshResourceErrors(action, resources, probe, defaults);
  }
}

function apiResourceErrors(request: Resources["api"][number], probe: boolean): string[] {
  const errors: string[] = [];
  if (request.preRequestScript?.trim() || request.postResponseScript?.trim()) errors.push("flow.apiScriptsUnsupported");
  if (request.bodyKind === "multipart-form-data") errors.push("flow.apiMultipartUnsupported");
  if (probe && !["GET", "HEAD"].includes(request.method?.toUpperCase() ?? "")) errors.push("flow.probeMethodError");
  return errors;
}

function databaseResourceErrors(action: FlowAction, resources: Resources, probe: boolean): string[] {
  const errors: string[] = [];
  const problem = sqlProblem(action.arguments.sql);
  if (problem) errors.push(problem);
  const limit = action.arguments.limit;
  // Keep deferred expressions for the owning authoring/runtime validators.
  if ("limit" in action.arguments && (typeof limit !== "number" ? limit === null : !Number.isInteger(limit) || limit < 1 || limit > 1000)) {
    errors.push("flow.inputTypeError");
  }
  if (probe && !resources.database.find((resource) => resource.id === action.resourceId)?.readOnly) {
    errors.push("flow.probeReadOnlyError");
  }
  return errors;
}

function sshResourceErrors(action: FlowAction, resources: Resources, probe: boolean, defaults?: Map<string, string>): string[] {
  const errors: string[] = [];
  if (!resources.connections.some((connection) => connection.id === action.connectionId)) errors.push("flow.missingConnection");
  if (sshInputsMissing(action, resources, defaults)) errors.push("flow.sshMissingInputs");
  if (probe) errors.push("flow.probeSshUnsupported");
  return errors;
}
