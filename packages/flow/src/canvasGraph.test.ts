import { describe, expect, it } from "vitest";
import type { FlowDefinition, FlowStep } from "@unfour/command-client";
import { definitionToGraph, END, graphToDefinition, insertCanvasStep, isCanvasConnectionValid, removalImpact, removeCanvasStep, START } from "./canvasGraph";
import { newStep } from "./model";

function fixture(): FlowDefinition {
  const steps = ["api", "condition", "database", "ssh", "waitUntil", "wait", "poll"].map((kind, index) => ({ ...newStep(kind, kind), id: `s${index}` }));
  steps[1] = { ...steps[1], kind: "condition", predicate: { left: { $ref: "/inputs/ready" }, op: "eq", right: true }, ifTrue: "s2", ifFalse: END };
  steps[2].next = "s4";
  steps[5].next = END;
  return { id: "flow", workspaceId: "ws", name: "test", revision: 7, inputs: [{ name: "token", type: "string", required: true, secret: true }], steps };
}

describe("Flow Canvas adapter", () => {
  it.each(["s0:next", "s1:true", "s1:false", "s2:next", "$start:next", "s6:next"])("inserts on %s without redirecting any other port", (port) => {
    const original = fixture();
    const oldEdges = definitionToGraph(original).edges;
    const oldTarget = oldEdges.find((edge) => edge.id === port)!.target;
    const result = insertCanvasStep(original, port, { ...newStep("wait", "Inserted"), id: "inserted" });
    const edges = definitionToGraph(result).edges;
    expect(edges.find((edge) => edge.id === port)?.target).toBe("inserted");
    expect(edges.find((edge) => edge.source === "inserted")?.target).toBe(oldTarget);
    for (const edge of oldEdges.filter((edge) => edge.id !== port)) expect(edges.find((next) => next.id === edge.id)?.target).toBe(edge.target);
    expect(graphToDefinition(result, definitionToGraph(result))).toEqual(result);
    expect(result.steps.filter((step) => step.id !== "inserted").map((step) => step.id)).toEqual(original.steps.map((step) => step.id));
  });
  it("inserts Condition with both branches continuing to the original destination", () => {
    const result = insertCanvasStep(fixture(), "s0:next", { ...newStep("condition", "Check"), id: "check" });
    expect(result.steps[1]).toMatchObject({ id: "check", ifTrue: "s1", ifFalse: "s1" });
    expect(definitionToGraph(result).edges.find((edge) => edge.source === "s1" && edge.sourceHandle === "false")?.target).toBe(END);
  });
  it("preserves absent legacy next properties and unknown extension fields on round trip", () => {
    const original = JSON.parse(JSON.stringify(fixture()));
    delete original.steps[0].next;
    delete original.steps[1].next;
    original.steps[6].legacyMetadata = { unchanged: true };
    const result = graphToDefinition(original, definitionToGraph(original));
    expect(JSON.stringify(result)).toBe(JSON.stringify(original));
  });
  it("reports references recursively and blocks deletion rather than leaving dangling references", () => {
    const original = fixture();
    const last = original.steps[6];
    if (last.kind !== "poll") throw new Error("fixture");
    last.probe.arguments = { nested: [{ $ref: "/steps/s2/rows/0" }], message: "${/steps/s2/rows}" };
    expect(removalImpact(original, "s2")).toEqual({ incoming: ["condition"], references: ["poll"] });
    expect(() => removeCanvasStep(original, "s2")).toThrow("REFERENCED_STEP");
    expect(removalImpact(original, "s0").references).toEqual([]);
  });
  it("round trips all node kinds, implicit/explicit next, branches, typed inputs and revision without layout", () => {
    const original = fixture();
    const graph = definitionToGraph(original, { s0: { x: 91, y: -42 } });
    expect(graph.nodes).toHaveLength(9);
    expect(graph.nodes[1].position).toEqual({ x: 91, y: -42 });
    expect(graphToDefinition(original, graph)).toEqual(original);
  });
  it("maps two distinct condition ports even when both lead to End", () => {
    const original = fixture();
    const graph = definitionToGraph(original);
    expect(graph.edges.filter((edge) => edge.source === "s1").map((edge) => [edge.sourceHandle, edge.target])).toEqual([["true", "s2"], ["false", END]]);
    const changed = { ...graph, edges: graph.edges.map((edge) => edge.source === "s1" ? { ...edge, target: END } : edge) };
    expect(graphToDefinition(original, changed).steps[1]).toMatchObject({ ifTrue: END, ifFalse: END });
  });
  it("edits branches independently and preserves condition's unused next", () => {
    const original = fixture();
    original.steps[1].next = "s6";
    const graph = definitionToGraph(original);
    graph.edges.find((edge) => edge.id === "s1:false")!.target = "s4";
    expect(graphToDefinition(original, graph).steps[1]).toMatchObject({ ifTrue: "s2", ifFalse: "s4", next: "s6" });
  });
  it("writes changed next and treats a removed edge as explicit End", () => {
    const original = fixture();
    const graph = definitionToGraph(original);
    graph.edges.find((edge) => edge.id === "s0:next")!.target = "s3";
    expect(graphToDefinition(original, graph).steps[0].next).toBe("s3");
    graph.edges = graph.edges.filter((edge) => edge.id !== "s0:next");
    expect(graphToDefinition(original, graph).steps[0].next).toBe(END);
  });
  it.each([
    ["s2", "s0", "next"], ["s2", "s2", "next"], ["s0", START, "next"],
    [END, "s2", "next"], [START, "s2", "next"], ["s0", "missing", "next"],
    ["s1", "s2", "next"], ["s0", "s2", "true"],
  ])("rejects illegal link %s -> %s (%s)", (source, target, sourceHandle) => {
    expect(isCanvasConnectionValid(fixture(), { source, target, sourceHandle, targetHandle: "in" })).toBe(false);
  });
  it("rejects duplicate outputs and invalid graphs at the reverse-conversion boundary", () => {
    const original = fixture();
    const graph = definitionToGraph(original);
    graph.edges.push({ ...graph.edges[1], id: "duplicate" });
    expect(() => graphToDefinition(original, graph)).toThrow("DUPLICATE_PORT");
    graph.edges.pop();
    graph.edges[1].target = "s0";
    expect(() => graphToDefinition(original, graph)).toThrow("INVALID_CONNECTION");
  });
  it("deletes a node without dangling targets and without reconnecting effects implicitly", () => {
    const original = fixture();
    const result = removeCanvasStep(original, "s2");
    expect(result.steps.map((step) => step.id)).not.toContain("s2");
    expect(result.steps[1]).toMatchObject({ ifTrue: END, ifFalse: END });
    expect(removeCanvasStep(original, "s1").steps[0].next).toBe(END);
    expect(definitionToGraph(removeCanvasStep(original, "s0")).edges[0].target).toBe("s1");
  });
  it("does not reorder execution when visual nodes are reordered or dragged", () => {
    const original = fixture();
    const graph = definitionToGraph(original);
    graph.nodes.reverse();
    expect(graphToDefinition(original, graph)).toEqual(original);
  });
  it("adding a node preserves implicit fallthrough and explicit End semantics", () => {
    const original = fixture();
    const added: FlowStep = { ...newStep("wait", "new"), id: "added" };
    const result = { ...original, steps: [...original.steps, added] };
    const graph = definitionToGraph(result);
    expect(graph.edges.find((edge) => edge.source === "s6")?.target).toBe("added");
    expect(graph.edges.find((edge) => edge.source === "s5")?.target).toBe(END);
    expect(graphToDefinition(result, graph)).toEqual(result);
  });
});
