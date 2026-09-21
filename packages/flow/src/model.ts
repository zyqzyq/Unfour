import type {
  FlowAction,
  FlowCapability,
  FlowPredicate,
  FlowStep,
  FlowDefinition,
  FlowInputDefinition,
} from "@unfour/command-client";
export type Resources = {
  api: { id: string; name: string; method?: string; bodyKind?: string; preRequestScript?: string | null; postResponseScript?: string | null }[];
  database: { id: string; name: string; readOnly?: boolean }[];
  ssh: { id: string; name: string }[];
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
export function maskInputs(values: unknown, definitions: FlowInputDefinition[], manual: string[]): unknown {
  const secret = new Set([...manual, ...definitions.filter((f) => f.secret).map((f) => f.name)].map((name) => name.toLowerCase()));
  const mask = (value: unknown): unknown => {
    if (Array.isArray(value)) return value.map(mask);
    if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).map(([key, child]) => [key, secret.has(key.toLowerCase()) || sensitiveKey(key) ? "••••••" : mask(child)]));
    return value;
  };
  return mask(values);
}
export function resourceErrors(definition: FlowDefinition, resources: Resources) {
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
    if (probe && action.capability === "database" && !resources.database.find((r) => r.id === action.resourceId)?.readOnly) add("flow.probeReadOnlyError");
    if (probe && action.capability === "ssh") add("flow.probeSshUnsupported");
    return errors;
  });
}
