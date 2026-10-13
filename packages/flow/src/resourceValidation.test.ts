import { describe, expect, it } from "vitest";
import type { FlowAction, FlowDefinition, FlowStep } from "@unfour/command-client";
import { environmentInputErrors, resourceErrors, type Resources } from "./model";

const resources: Resources = {
  api: [{ id: "api", name: "API", method: "POST", bodyKind: "multipart-form-data", postResponseScript: "script" }],
  database: [{ id: "db", name: "DB", readOnly: false }],
  connections: [{ id: "connection", name: "SSH" }],
  ssh: [{ id: "task", name: "Task", detail: {
    task: { id: "task", workspaceId: "w", name: "Task", description: "", sortOrder: 0, createdAt: "", updatedAt: "", deletedAt: null },
    steps: [], localBinding: null, detectedInputs: ["Host"],
  } }],
};

function definition(action: FlowAction, kind: "action" | "poll" | "waitUntil" = "action"): FlowDefinition {
  const base = { id: "step", name: "Step", timeoutMs: 1000 };
  const predicate = { left: true, op: "eq" as const, right: true };
  const step: FlowStep = kind === "action" ? { ...base, kind, action }
    : kind === "poll" ? { ...base, kind, probe: action, predicate, intervalMs: 10, maxAttempts: 2 }
      : { ...base, kind, probe: action, successWhen: predicate, intervalMs: 10 };
  return { id: "flow", workspaceId: "w", name: "Flow", revision: 1, inputs: [], steps: [step] };
}
const ssh: FlowAction = { capability: "ssh", resourceId: "task", connectionId: "connection", arguments: { workspaceDefaults: true } };
const keys = (flow: FlowDefinition, source = resources, defaults?: Map<string, string>) =>
  resourceErrors(flow, source, defaults).map((error) => error.key);

describe("resource validation order and capability boundaries", () => {
  it.each(["action", "poll", "waitUntil"] as const)("preserves %s error ordering and missing-resource short circuit", (kind) => {
    const flow = definition({ capability: "api", resourceId: "api", arguments: {} }, kind);
    const expected = ["flow.apiScriptsUnsupported", "flow.apiMultipartUnsupported"];
    if (kind !== "action") expected.push("flow.probeMethodError");
    expect(keys(flow)).toEqual(expected);
    expect(resourceErrors(flow, { ...resources, api: [] })).toEqual([{ stepId: "step", name: "Step", key: "flow.missingResource" }]);
    expect(keys(definition({ ...ssh, resourceId: "missing", connectionId: "missing", arguments: {} }, kind)))
      .toEqual(["flow.missingResource"]);
  });

  it("keeps errors in step order and ignores non-executing steps", () => {
    const flow = definition({ ...ssh, connectionId: "missing", arguments: {} }, "poll");
    flow.steps.push({ id: "wait", name: "Wait", kind: "wait", timeoutMs: 1000, durationMs: 1 });
    flow.steps.push({ ...definition({ capability: "database", resourceId: "db", arguments: { limit: 0 } }, "waitUntil").steps[0], id: "db", name: "DB" });
    expect(resourceErrors(flow, resources)).toEqual([
      { stepId: "step", name: "Step", key: "flow.missingConnection" },
      { stepId: "step", name: "Step", key: "flow.sshMissingInputs" },
      { stepId: "step", name: "Step", key: "flow.probeSshUnsupported" },
      { stepId: "db", name: "DB", key: "flow.sqlRequired" },
      { stepId: "db", name: "DB", key: "flow.inputTypeError" },
      { stepId: "db", name: "DB", key: "flow.probeReadOnlyError" },
    ]);
  });

  it.each([null, 0, -1, 1.5, 1001, NaN])("rejects invalid literal query limit %s", (limit) => {
    expect(keys(definition({ capability: "database", resourceId: "db", arguments: { sql: "SELECT 1", limit } })))
      .toEqual(["flow.inputTypeError"]);
  });

  it.each([undefined, 1, 1000, { $ref: "/inputs/limit" }, "${/inputs/limit}"])("preserves deferred or valid limit %s", (limit) => {
    expect(keys(definition({ capability: "database", resourceId: "db", arguments: { sql: "SELECT 1", limit } })))
      .toEqual([]);
  });

  it("permits read-only database and GET/HEAD API probes without broadening script support", () => {
    const db = definition({ capability: "database", resourceId: "db", arguments: { sql: "SELECT 1" } }, "poll");
    expect(keys(db, { ...resources, database: [{ id: "db", name: "DB", readOnly: true }] })).toEqual([]);
    for (const method of ["get", "HEAD"]) {
      const api = definition({ capability: "api", resourceId: "api", arguments: {} }, "waitUntil");
      expect(keys(api, { ...resources, api: [{ id: "api", name: "API", method, preRequestScript: "  " }] })).toEqual([]);
    }
  });
});

describe("SSH resource validation defaults", () => {
  it("defers unavailable defaults and adds only errors introduced by the selected environment", () => {
    const flow = definition(ssh);
    expect(keys(flow)).toEqual([]);
    expect(environmentInputErrors(flow, resources, new Map())).toEqual([{ stepId: "step", name: "Step", key: "flow.sshMissingInputs" }]);
    expect(environmentInputErrors(flow, resources, new Map([["host", "value"]]))).toEqual([]);
  });

  it("keeps explicit blank inputs invalid even when defaults are available", () => {
    const flow = definition({ ...ssh, arguments: { workspaceDefaults: true, inputs: { Host: " " } } });
    expect(keys(flow, resources, new Map([["host", "value"]]))).toEqual(["flow.sshMissingInputs"]);
    expect(environmentInputErrors(flow, resources, new Map())).toEqual([]);
  });

  it("preserves exact input names, dynamic bindings, and explicit non-string values", () => {
    expect(keys(definition({ ...ssh, arguments: { inputs: { host: "wrong case" } } }))).toEqual(["flow.sshMissingInputs"]);
    for (const inputs of [{ $ref: "/inputs/ssh" }, { Host: false }, { Host: 0 }]) {
      expect(keys(definition({ ...ssh, arguments: { inputs } }))).toEqual([]);
    }
    expect(keys(definition(ssh), { ...resources, ssh: [{ id: "task", name: "Task" }] })).toEqual([]);
  });
});
