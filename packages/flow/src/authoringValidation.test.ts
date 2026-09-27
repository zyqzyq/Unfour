import { expect, it } from "vitest";
import type { FlowDefinition, FlowStep } from "@unfour/command-client";
import { authoringProblems } from "./authoringValidation";
import { newStep } from "./model";

const definition = (step: FlowStep, name = "Flow"): FlowDefinition => ({ id: "f", workspaceId: "w", name, revision: 1, inputs: [], steps: [step] });
it("checks UTF-8 name bounds and the runtime timeout and wait relationship", () => {
  const step = newStep("wait", "Wait");
  expect(authoringProblems(definition(step))).toEqual([]);
  expect(authoringProblems(definition(step, "界".repeat(67)))).toEqual([{ key: "flow.nameBounds" }]);
  expect(authoringProblems(definition({ ...step, timeoutMs: 0 })).map((p) => p.key)).toContain("flow.timeoutBounds");
  expect(authoringProblems(definition({ ...step, kind: "wait", durationMs: 60000 })).map((p) => p.key)).toContain("flow.waitBounds");
  expect(authoringProblems(definition({ ...step, kind: "wait", durationMs: 0 }))).toEqual([]);
});
it("allows uncapped Wait Until while rejecting fractional and out-of-range bounds", () => {
  const step = newStep("waitUntil", "Ready");
  if (step.kind !== "waitUntil") throw new Error("Expected Wait Until");
  expect(authoringProblems(definition(step))).toEqual([]);
  for (const intervalMs of [9, 60001, 10.5]) expect(authoringProblems(definition({ ...step, intervalMs })).map((p) => p.key)).toContain("flow.intervalBounds");
  for (const maxAttempts of [0, 1001, 1.5]) expect(authoringProblems(definition({ ...step, maxAttempts })).map((p) => p.key)).toContain("flow.attemptBounds");
  expect(authoringProblems(definition({ ...step, intervalMs: 10, maxAttempts: 1 }))).toEqual([]);
});
