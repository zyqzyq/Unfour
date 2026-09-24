import { expect, it } from "vitest";
import { inputDefaults, inputErrors, isInRightOperand, isPredicate, maskInputs, newStep, resourceErrors } from "./model";
import type { FlowDefinition, FlowInputDefinition } from "@unfour/command-client";

const definitions: FlowInputDefinition[] = [
  { name: "count", type: "number", required: true, secret: false, default: 0 },
  { name: "enabled", type: "boolean", required: true, secret: false, default: false },
  { name: "token", type: "string", required: true, secret: true },
];
it("keeps false and zero defaults and validates required and typed inputs", () => {
  expect(inputDefaults(definitions)).toEqual({ count: 0, enabled: false });
  expect(inputErrors(definitions, { count: 0, enabled: false, token: "x" })).toEqual([]);
  expect(inputErrors(definitions, { count: "0", enabled: false })).toEqual([
    { name: "count", key: "flow.inputTypeError" },
    { name: "token", key: "flow.inputRequiredError" },
  ]);
});
it("masks declared, manual and sensitive nested keys without hiding safe context", () => {
  expect(maskInputs({ token: "secret", manual: "secret", headers: { Authorization: "secret", "x-api-key": "secret" }, count: 2 }, definitions, ["manual"]))
    .toEqual({ token: "••••••", manual: "••••••", headers: { Authorization: "••••••", "x-api-key": "••••••" }, count: 2 });
});
it("creates timeout-bounded Wait Until with no attempt cap", () => {
  expect(newStep("waitUntil", "Ready")).toMatchObject({ kind: "waitUntil", maxAttempts: null, intervalMs: 1000, probeErrorPolicy: "failImmediately", intervalStrategy: "fixed", successWhen: { left: { $ref: "/probe/body/ready" } } });
});
it("treats in right operands as literal arrays or configured $ref values", () => {
  expect(isInRightOperand([400, 404])).toBe(true);
  expect(isInRightOperand({ $ref: "/inputs/allowedStatuses" })).toBe(true);
  expect(isInRightOperand({ $ref: "" })).toBe(false);
  expect(isInRightOperand("400")).toBe(false);
  expect(isInRightOperand(400)).toBe(false);
  expect(isInRightOperand(true)).toBe(false);
  expect(isPredicate({ left: 400, op: "in", right: [400, 404] })).toBe(true);
  expect(isPredicate({ left: { $ref: "/probe/status" }, op: "in", right: { $ref: "/inputs/allowedStatuses" } })).toBe(true);
  expect(isPredicate({ left: 400, op: "in", right: { $ref: "" } })).toBe(false);
  expect(isPredicate({ left: 400, op: "in", right: "400" })).toBe(false);
});
it("preflights metadata for probes, API scripts and missing resources", () => {
  const step = newStep("waitUntil", "Ready");
  if (step.kind !== "waitUntil") throw new Error("Expected waitUntil");
  const flow: FlowDefinition = { id: "f", workspaceId: "w", name: "f", revision: 1, inputs: [], steps: [{ ...step, probe: { ...step.probe, resourceId: "api" } }] };
  const resources = { api: [{ id: "api", name: "API", method: "POST", bodyKind: "multipart-form-data", preRequestScript: "script" }], database: [], ssh: [], connections: [] };
  expect(resourceErrors(flow, resources).map((e) => e.key)).toEqual(["flow.apiScriptsUnsupported", "flow.apiMultipartUnsupported", "flow.probeMethodError"]);
  expect(resourceErrors(flow, { ...resources, api: [] }).map((e) => e.key)).toEqual(["flow.missingResource"]);
  flow.steps = [{ ...step, probe: { capability: "database", resourceId: "db", connectionId: null, arguments: {} } }];
  expect(resourceErrors(flow, { ...resources, database: [{ id: "db", name: "DB", readOnly: false }] }).map((e) => e.key)).toEqual(["flow.sqlRequired", "flow.probeReadOnlyError"]);
  flow.steps = [{ ...newStep("ssh", "Deploy"), kind: "action", action: { capability: "ssh", resourceId: "task", connectionId: "deleted", arguments: {} } }];
  expect(resourceErrors(flow, { ...resources, ssh: [{ id: "task", name: "Task" }] }).map((e) => e.key)).toEqual(["flow.missingConnection"]);
});


it("rejects reference objects with extra fields on either predicate operand", () => {
  expect(isInRightOperand({ $ref: "/inputs/list", extra: 1 })).toBe(false);
  for (const op of ["eq", "in"]) {
    expect(isPredicate({ left: { $ref: "/inputs/value", extra: 1 }, op, right: [] })).toBe(false);
    expect(isPredicate({ left: true, op, right: { $ref: "/inputs/list", extra: 1 } })).toBe(false);
  }
  expect(isPredicate({ left: [{ $ref: "/inputs/value", extra: 1 }], op: "eq", right: [] })).toBe(false);
});
