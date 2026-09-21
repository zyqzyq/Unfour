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
  return [
    ...inputs.map((input) => ({ label: input.name, path: ["inputs", input.name] })),
    ...before.flatMap((step) => [[], ...outputFields(step).map((field) => [field])].map((suffix) => ({ label: [step.name, ...suffix].join(" · "), path: ["steps", step.id, ...suffix] }))),
    ...(probe && (probe.kind === "poll" || probe.kind === "waitUntil") ? [[], ...outputFields({ ...probe, kind: "action", action: probe.probe }).map((field) => [field])].map((suffix) => ({ label: suffix.join(" · "), path: ["probe", ...suffix] })) : []),
  ];
}
