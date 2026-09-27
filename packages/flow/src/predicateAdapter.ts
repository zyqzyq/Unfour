import type { FlowPredicate } from "@unfour/command-client";

export type PredicateOperator = Exclude<FlowPredicate["op"], "in"> | "contains" | "true" | "false";
export type PredicateModel = { left: unknown; operator: PredicateOperator; right: unknown };

export function decodePredicate(value: FlowPredicate): PredicateModel {
  if (value.op === "in") return { left: value.right, operator: "contains", right: value.left };
  if (value.op === "eq" && typeof value.right === "boolean") return { left: value.left, operator: value.right ? "true" : "false", right: value.right };
  return { left: value.left, operator: value.op, right: value.right };
}

export function encodePredicate(model: PredicateModel): FlowPredicate {
  if (model.operator === "contains") return { left: model.right, op: "in", right: model.left };
  if (model.operator === "true" || model.operator === "false") return { left: model.left, op: "eq", right: model.operator === "true" };
  return { left: model.left, op: model.operator, right: model.right };
}
