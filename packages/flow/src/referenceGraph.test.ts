import { expect, it } from "vitest";
import type { FlowStep } from "@unfour/command-client";
import { guaranteedUpstream, unsafeReferences } from "./referenceGraph";
import { newStep } from "./model";

const wait = (id: string, next: string | null = null): FlowStep => ({ ...newStep("wait", id), id, next });
const branch = (id: string, ifTrue: string, ifFalse: string): FlowStep => ({ ...newStep("condition", id), id, kind: "condition", predicate: { left: true, op: "eq", right: true }, ifTrue, ifFalse, next: null });

it("distinguishes malformed objects and interpolations from unsafe paths", () => {
  for (const value of [{ $ref: 2 }, { $ref: "/inputs/a", extra: true }, { $ref: "/inputs/a~2" }, "${}", "${inputs/a}", "${/inputs/a", "${/inputs/a} ${/other/b}"]) {
    const step = branch("check", "$end", "$end");
    if (step.kind !== "condition") throw new Error("fixture");
    step.predicate.left = value;
    expect(unsafeReferences([step]).map((p) => p.key)).toContain("flow.invalidReference");
    step.predicate.left = "${/inputs/a~1b} ${/inputs/c~0d}";
    expect(unsafeReferences([step])).toEqual([]);
    step.predicate.left = { $ref: "/steps/missing" };
    expect(unsafeReferences([step])[0].key).toBe("flow.unsafeReference");
  }
});

it("intersects both branches at a join, and keeps branch-local dominators", () => {
  const steps = [wait("start"), branch("split", "a", "b"), wait("a", "a2"), wait("a2", "join"), wait("b", "join"), wait("join"), wait("end")];
  const safe = guaranteedUpstream(steps);
  expect([...safe.get("a2")!]).toEqual(["start", "split", "a"]);
  expect([...safe.get("b")!]).toEqual(["start", "split"]);
  expect([...safe.get("join")!]).toEqual(["start", "split"]);
  expect([...safe.get("end")!]).toEqual(["start", "split", "join"]);
});

it("honors jumps, end, identical targets, nested branches and unreachable predecessors", () => {
  const steps = [branch("outer", "inner", "$end"), branch("inner", "a", "join"), wait("a", "join"), wait("unreachable", "join"), branch("join", "tail", "tail"), wait("tail")];
  expect([...guaranteedUpstream(steps).get("join")!]).toEqual(["outer", "inner"]);
  expect([...guaranteedUpstream(steps).get("tail")!]).toEqual(["outer", "inner", "join"]);
  expect([...guaranteedUpstream(steps).get("unreachable")!]).toEqual([]);
  expect([...guaranteedUpstream([wait("entry", "last"), wait("skipped"), wait("last")]).get("last")!]).toEqual(["entry"]);
});

it("checks nested refs and interpolations, self/future/missing outputs and probe scope", () => {
  const action: FlowStep = { ...newStep("api", "Use"), id: "use", kind: "action", action: { capability: "api", resourceId: "api", connectionId: null, arguments: { body: [{ $ref: "/steps/a/body" }, "${/steps/b/body}", "${/steps/use/body}", "${/steps/missing}", "${/probe/body}", "${/steps/split/matched}"] } } };
  const steps = [branch("split", "a", "b"), wait("a", "use"), wait("b", "use"), action];
  expect(unsafeReferences(steps).map((item) => item.pointer)).toEqual(["/steps/a/body", "/steps/b/body", "/steps/use/body", "/steps/missing", "/probe/body"]);
  for (const kind of ["poll", "waitUntil"]) {
    const probe = newStep(kind, kind);
    expect(unsafeReferences([probe])).toEqual([]);
    if (probe.kind !== "poll" && probe.kind !== "waitUntil") throw new Error("fixture");
    probe.probe.arguments = { url: "${/probe/body/url}" };
    expect(unsafeReferences([probe])).toHaveLength(1);
  }
});

it("updates availability immediately after rerouting a branch", () => {
  const steps = [branch("split", "a", "a"), wait("a"), wait("join")];
  expect(guaranteedUpstream(steps).get("join")?.has("a")).toBe(true);
  steps[0] = branch("split", "a", "join");
  expect(guaranteedUpstream(steps).get("join")?.has("a")).toBe(false);
});
