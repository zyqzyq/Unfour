import { expect, it } from "vitest";
import type { FlowDefinition } from "../../types/flow";
import { handleFlowMock } from "./flow";
import { UNHANDLED } from "./types";

it("matches the persisted revision and tombstone contract in authoring preview", async () => {
  const input: FlowDefinition = { id: "", workspaceId: "preview-flow", name: "Preview", revision: 0,
    inputs: [], steps: [{ id: "wait", name: "Wait", kind: "wait", durationMs: 0, timeoutMs: 1000, next: null }] };
  const saved = await handleFlowMock<FlowDefinition>("flow_save", { input });
  if (saved === UNHANDLED) throw new Error("flow_save must be handled");
  const revised = await handleFlowMock<FlowDefinition>("flow_save", { input: saved });
  if (revised === UNHANDLED) throw new Error("flow_save must be handled");
  const args = { workspaceId: saved.workspaceId, flowId: saved.id, expectedRevision: saved.revision };
  expect(() => handleFlowMock("flow_delete", args)).toThrow();
  expect(() => handleFlowMock("flow_save", { input: saved })).toThrow();
  expect(() => handleFlowMock("flow_delete", { ...args, workspaceId: "foreign", expectedRevision: revised.revision })).toThrow();
  handleFlowMock("flow_delete", { ...args, expectedRevision: revised.revision });
  expect(await handleFlowMock("flow_list", { workspaceId: saved.workspaceId })).toEqual([]);
  expect(() => handleFlowMock("flow_get", args)).toThrow();
  expect(() => handleFlowMock("flow_save", { input: revised })).toThrow();
  expect(() => handleFlowMock("flow_delete", { ...args, expectedRevision: revised.revision + 1 })).toThrow();
});
