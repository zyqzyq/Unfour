import type { FlowPredicate, FlowStep } from "@unfour/command-client";
import type { Resources } from "./model";
import { decodePredicate } from "./predicateAdapter";

export function predicateSummary(predicate: FlowPredicate, steps: FlowStep[], t: (key: string) => string): string {
  const model = decodePredicate(predicate);
  const operand = (value: unknown): string => {
    if (value && typeof value === "object" && "$ref" in value && typeof value.$ref === "string") {
      const [root, id, ...fields] = value.$ref.slice(1).split("/").map((part) => part.replace(/~1/g, "/").replace(/~0/g, "~"));
      if (root === "steps") return [steps.find((step) => step.id === id)?.name ?? id, fields.join(".")].filter(Boolean).join(" · ");
      return [id, ...fields].filter((part) => part !== undefined).join(".");
    }
    if (typeof value === "string" && value.includes("${")) return t("flow.value.variable");
    if (value !== null && typeof value === "object") return t("flow.value.json");
    return JSON.stringify(value) ?? "";
  };
  return [operand(model.left), t(`flow.operatorLabels.${model.operator}`), ...(["true", "false"].includes(model.operator) ? [] : [operand(model.right)])].join(" ");
}

export function nodeSummary(step: FlowStep, steps: FlowStep[], resources: Resources | undefined, t: (key: string) => string) {
  if (step.kind === "condition") return predicateSummary(step.predicate, steps, t);
  if (step.kind === "wait") return `${step.durationMs / 1000}s`;
  const action = step.kind === "action" ? step.action : step.probe;
  const resource = resources?.[action.capability].find((item) => item.id === action.resourceId)?.name ?? t("flow.selectResource");
  if (step.kind === "action") return `${t(`flow.${action.capability}`)} · ${resource}`;
  return `${t("flow.probe")} ${resource} · ${t("flow.every")} ${step.intervalMs / 1000}s · ${t("flow.until")} ${predicateSummary(step.kind === "poll" ? step.predicate : step.successWhen, steps, t)}`;
}
