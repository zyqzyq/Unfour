import { expect, it } from "vitest";
import type { FlowPredicate } from "@unfour/command-client";
import { decodePredicate, encodePredicate } from "./predicateAdapter";
import { predicateSummary, nodeSummary } from "./nodeSummary";
import { newStep } from "./model";

it("round trips every legacy operator and operand without rewriting refs or interpolation", () => {
  const predicates: FlowPredicate[] = [
    ...(["eq", "ne", "gt", "ge", "lt", "le"] as const).map((op) => ({ left: { $ref: "/probe/body/value" }, op, right: 42 })),
    { left: { $ref: "/probe/status" }, op: "in", right: [200, 204] },
    { left: "${/inputs/value}", op: "in", right: { $ref: "/inputs/allowed" } },
    { left: { $ref: "/steps/wait/result/body/ready" }, op: "eq", right: true },
    { left: null, op: "eq", right: false },
  ];
  for (const value of predicates) expect(encodePredicate(decodePredicate(value))).toEqual(value);
});

it("encodes array contains by reversing operands and boolean shortcuts as equality", () => {
  expect(encodePredicate({ left: { $ref: "/inputs/list" }, operator: "contains", right: "ready" })).toEqual({ left: "ready", op: "in", right: { $ref: "/inputs/list" } });
  expect(encodePredicate({ left: { $ref: "/probe/body/ready" }, operator: "false", right: null })).toEqual({ left: { $ref: "/probe/body/ready" }, op: "eq", right: false });
});

it("summarizes business fields, resources and intervals without reference JSON", () => {
  const t = (key: string) => ({ "flow.operatorLabels.eq": "equals", "flow.probe": "Poll", "flow.every": "every", "flow.until": "until" })[key] ?? key;
  const predicate: FlowPredicate = { left: { $ref: "/probe/body/status" }, op: "eq", right: "ready" };
  expect(predicateSummary(predicate, [], t)).toBe('body.status equals "ready"');
  const step = newStep("waitUntil", "Ready");
  if (step.kind !== "waitUntil") throw new Error("fixture");
  step.successWhen = predicate; step.intervalMs = 5000; step.probe.resourceId = "status";
  expect(nodeSummary(step, [step], { api: [{ id: "status", name: "Get status" }], ssh: [], database: [], connections: [] }, t)).toBe('Poll Get status · every 5s · until body.status equals "ready"');
});
