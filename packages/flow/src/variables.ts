import type { FlowInputDefinition, FlowStep } from "@unfour/command-client";

export type Variable = { label: string; path: string[] };
export const variableReference = (path: string[]) => ({ $ref: "/" + path.map((part) => part.replace(/~/g, "~0").replace(/\//g, "~1")).join("/") });
export function variablesFor(inputs: FlowInputDefinition[], before: FlowStep[], probe?: FlowStep): Variable[] {
  const outputFields = (step: FlowStep): string[] => {
    if (step.kind === "waitUntil") return ["result", "attempts", "elapsedMs"];
    if (step.kind === "poll") return outputFields({ ...step, kind: "action", action: step.probe });
    if (step.kind === "condition") return ["matched"];
    if (step.kind === "wait") return ["waitedMs"];
    return step.action.capability === "api" ? ["status", "headers", "body", "durationMs"] : step.action.capability === "database" ? ["columns", "rows", "affectedRows", "durationMs"] : ["runId", "status", "log"];
  };
  const outputPaths = (step: FlowStep): string[][] => [[], ...outputFields(step).map((field) => [field]),
    ...(step.kind === "waitUntil" ? outputFields({ ...step, kind: "action", action: step.probe }).map((field) => ["result", field]) : []),
  ];
  return [
    ...inputs.map((input) => ({ label: input.name, path: ["inputs", input.name] })),
    ...before.flatMap((step) => outputPaths(step).map((suffix) => ({ label: [step.name, suffix.join(".")].filter(Boolean).join(" · "), path: ["steps", step.id, ...suffix] }))),
    ...(probe && (probe.kind === "poll" || probe.kind === "waitUntil") ? outputPaths({ ...probe, kind: "action", action: probe.probe }).map((suffix) => ({ label: [probe.name, suffix.join(".")].filter(Boolean).join(" · "), path: ["probe", ...suffix] })) : []),
  ];
}
