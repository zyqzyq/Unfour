import type {
  FlowAction,
  FlowCapability,
  FlowPredicate,
  FlowStep,
  FlowDefinition,
  FlowInputDefinition,
} from "@unfour/command-client";
import { sqlProblem } from "./actionAuthoring";
import type { SshTaskDetail } from "@unfour/command-client";
export type Resources = {
  api: { id: string; name: string; url?: string; headersJson?: string; queryJson?: string; body?: string | null; method?: string; bodyKind?: string; preRequestScript?: string | null; postResponseScript?: string | null }[];
  database: { id: string; name: string; readOnly?: boolean }[];
  ssh: { id: string; name: string; detail?: SshTaskDetail }[];
  connections: { id: string; name: string }[];
};
export const emptyAction = (
  capability: FlowCapability = "api",
): FlowAction => ({
  capability,
  resourceId: "",
  connectionId: null,
  arguments:
    capability === "database"
      ? { sql: "SELECT 1" }
      : capability === "ssh"
        ? { inputs: {} }
        : {},
});
export function newStep(kind: string, name: string): FlowStep {
  const base = {
    id: `step_${crypto.randomUUID().replace(/-/g, "").slice(0, 12)}`,
    name,
    timeoutMs: 60000,
    next: null,
  };
  const predicate: FlowPredicate = {
    left: { $ref: kind === "poll" || kind === "waitUntil" ? "/probe/body/ready" : "/inputs/ready" },
    op: "eq",
    right: true,
  };
  if (kind === "condition")
    return { ...base, kind, predicate, ifTrue: "$end", ifFalse: "$end" };
  if (kind === "waitUntil")
    return { ...base, kind, probe: emptyAction(), successWhen: predicate, failureWhen: null, intervalMs: 1000, maxAttempts: null, probeErrorPolicy: "failImmediately", intervalStrategy: "fixed" };
  if (kind === "poll")
    return {
      ...base,
      kind,
      probe: emptyAction(),
      predicate,
      intervalMs: 1000,
      maxAttempts: 30,
    };
  if (kind === "wait") return { ...base, kind, durationMs: 1000 };
  return {
    ...base,
    kind: "action",
    action: emptyAction(kind as FlowCapability),
  };
}

// Older browser drafts may still contain v1 names; Rust normalizes persisted definitions.
export function normalizeInputs(inputs: (FlowInputDefinition | string)[]): FlowInputDefinition[] {
  return inputs.map((input) => typeof input === "string" ? { name: input, type: "json", required: true, secret: false } : input);
}
export function inputDefaults(inputs: FlowInputDefinition[]): Record<string, unknown> {
  return Object.fromEntries(inputs.filter((input) => input.default !== undefined && !input.secret && !sensitiveKey(input.name)).map((input) => [input.name, input.default]));
}
export function inputDefinitionErrors(definitions: FlowInputDefinition[]) {
  const names = new Set<string>();
  return definitions.flatMap((field) => {
    const invalidName = !field.name.trim() || field.name.length > 200 || names.has(field.name);
    names.add(field.name);
    if (invalidName) return [{ name: field.name, key: "flow.inputNameError" }];
    if (field.default !== undefined && (field.secret || sensitiveKey(field.name))) return [{ name: field.name, key: "flow.secretDefaultHelp" }];
    return inputErrors([{ ...field, required: false }], { [field.name]: field.default });
  });
}
export function inputErrors(definitions: FlowInputDefinition[], values: Record<string, unknown>) {
  return definitions.flatMap((field) => {
    const value = values[field.name];
    if (value === undefined) return field.required ? [{ name: field.name, key: "flow.inputRequiredError" }] : [];
    if (field.type !== "json" && (typeof value !== field.type || (field.type === "number" && !Number.isFinite(value)))) return [{ name: field.name, key: "flow.inputTypeError" }];
    return [];
  });
}
export function sensitiveKey(key: string) {
  return /authorization|cookie|password|passwd|secret|token|api.?key|private.?key|passphrase|credential/i.test(key);
}

function isReferenceOperand(value: unknown): value is { $ref: string } {
  return Boolean(value && typeof value === "object" && !Array.isArray(value) && Object.keys(value).length === 1 && "$ref" in value && typeof value.$ref === "string");
}

export function validReferences(value: unknown): boolean {
  if (!value || typeof value !== "object") return true;
  if ("$ref" in value) return isReferenceOperand(value) && value.$ref.startsWith("/");
  return Object.values(value).every(validReferences);
}

/** Authoring-time `in` right operand: a literal array or a configured `$ref`. Engine resolves the ref before requiring an array. */
export function isInRightOperand(value: unknown): boolean {
  return Array.isArray(value) || (isReferenceOperand(value) && Boolean(value.$ref));
}

export function isPredicate(value: unknown): value is FlowPredicate {
  return Boolean(
    value &&
      typeof value === "object" &&
      "left" in value &&
      "right" in value &&
      "op" in value &&
      validReferences(value.left) && validReferences(value.right) &&
      ["eq", "ne", "gt", "ge", "lt", "le", "in"].includes(String(value.op)) &&
      (value.op !== "in" || isInRightOperand(value.right)),
  );
}
export function maskInputs(values: unknown, definitions: FlowInputDefinition[], manual: string[]): unknown {
  const secret = new Set([...manual, ...definitions.filter((f) => f.secret).map((f) => f.name)].map((name) => name.toLowerCase()));
  const mask = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(mask);
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, secret.has(key.toLowerCase()) || sensitiveKey(key) ? "••••••" : mask(child)]));
    return value;
  };
  return mask(values);
}
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
    const errors: { name: string; key: string }[] = [];
    const add = (key: string) => errors.push({ name: step.name, key });
    if (!resources[action.capability].some((r) => r.id === action.resourceId)) { add("flow.missingResource"); return errors; }
    if (action.capability === "ssh" && !resources.connections.some((c) => c.id === action.connectionId)) add("flow.missingConnection");
    if (action.capability === "api") {
      const request = resources.api.find((r) => r.id === action.resourceId)!;
      if (request.preRequestScript?.trim() || request.postResponseScript?.trim()) add("flow.apiScriptsUnsupported");
      if (request.bodyKind === "multipart-form-data") add("flow.apiMultipartUnsupported");
      if (probe && !["GET", "HEAD"].includes(request.method?.toUpperCase() ?? "")) add("flow.probeMethodError");
    }
    if (action.capability === "database") {
      const problem = sqlProblem(action.arguments.sql);
      if (problem) add(problem);
      if ("limit" in action.arguments && (typeof action.arguments.limit !== "number" ? action.arguments.limit === null : !Number.isInteger(action.arguments.limit) || action.arguments.limit < 1 || action.arguments.limit > 1000)) add("flow.inputTypeError");
    }
    if (action.capability === "ssh" && sshInputsMissing(action, resources, defaults)) add("flow.sshMissingInputs");
    if (probe && action.capability === "database" && !resources.database.find((r) => r.id === action.resourceId)?.readOnly) add("flow.probeReadOnlyError");
    if (probe && action.capability === "ssh") add("flow.probeSshUnsupported");
    return errors;
  });
}
