import type {
  FlowAction,
  FlowCapability,
  FlowPredicate,
  FlowStep,
} from "@unfour/command-client";
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
    left: { $ref: kind === "poll" ? "/probe/body/ready" : "/inputs/ready" },
    op: "eq",
    right: true,
  };
  if (kind === "condition")
    return { ...base, kind, predicate, ifTrue: "$end", ifFalse: "$end" };
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
