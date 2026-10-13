import type {
  FlowAction,
  FlowCapability,
  FlowPredicate,
  FlowStep,
  FlowInputDefinition,
} from "@unfour/command-client";
export type { Resources } from "./resourceValidation";
export { environmentInputErrors, resourceErrors } from "./resourceValidation";
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
  return /authorization|cookie|password|passwd|secret|token|apikey|privatekey|passphrase|credential|licensekey/i.test(key.replace(/[_ -]/g, ""));
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
