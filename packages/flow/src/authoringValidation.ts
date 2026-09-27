import type { FlowDefinition } from "@unfour/command-client";

/** Mirror existing runtime bounds so users can fix fields before saving. */
export function authoringProblems(definition: FlowDefinition) {
  const problems: { stepId?: string; key: string }[] = [];
  if (!definition.name.trim() || new TextEncoder().encode(definition.name).length > 200) problems.push({ key: "flow.nameBounds" });
  const integer = (value: number, min: number, max: number) => Number.isInteger(value) && value >= min && value <= max;
  for (const step of definition.steps) {
    const add = (key: string) => problems.push({ stepId: step.id, key });
    if (!integer(step.timeoutMs, 1, 3600000)) add("flow.timeoutBounds");
    if (step.kind === "wait" && !integer(step.durationMs, 0, step.timeoutMs - 1)) add("flow.waitBounds");
    if (step.kind === "poll" || step.kind === "waitUntil") {
      if (!integer(step.intervalMs, 10, 60000)) add("flow.intervalBounds");
      if ((step.kind === "poll" || step.maxAttempts != null) && !integer(step.maxAttempts!, 1, 1000)) add("flow.attemptBounds");
    }
  }
  return problems;
}
