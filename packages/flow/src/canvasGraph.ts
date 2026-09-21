import type { FlowDefinition, FlowStep } from "@unfour/command-client";
import type { Connection, Edge, Node, XYPosition } from "@xyflow/react";

export const START = "$start";
export const END = "$end";
export type CanvasNode = Node<{ step?: FlowStep; virtual?: "start" | "end"; order: number; status?: string }, "flowStep">;
export type CanvasGraph = { nodes: CanvasNode[]; edges: Edge[] };
export type CanvasLayout = Record<string, XYPosition>;

export function definitionToGraph(definition: FlowDefinition, layout: CanvasLayout = {}): CanvasGraph {
  const nodes: CanvasNode[] = [
    { id: START, type: "flowStep", position: layout[START] ?? { x: 40, y: 100 }, data: { virtual: "start", order: -1 }, deletable: false },
    ...definition.steps.map((step, order): CanvasNode => ({
      id: step.id, type: "flowStep", position: layout[step.id] ?? { x: 280 + order * 240, y: 100 }, data: { step, order },
    })),
    { id: END, type: "flowStep", position: layout[END] ?? { x: 280 + definition.steps.length * 240, y: 100 }, data: { virtual: "end", order: definition.steps.length }, deletable: false },
  ];
  const edge = (source: string, sourceHandle: string, target: string): Edge => ({
    id: `${source}:${sourceHandle}`, source, sourceHandle, target, targetHandle: "in", deletable: source !== START,
  });
  const edges = [edge(START, "next", definition.steps[0]?.id ?? END)];
  definition.steps.forEach((step, index) => {
    if (step.kind === "condition") {
      edges.push(edge(step.id, "true", step.ifTrue), edge(step.id, "false", step.ifFalse));
    } else {
      const next = edge(step.id, "next", step.next ?? definition.steps[index + 1]?.id ?? END);
      next.style = step.next == null ? { strokeDasharray: "5 4" } : undefined;
      edges.push(next);
    }
  });
  return { nodes, edges };
}

// Array order is an engine constraint, not the node's on-screen position.
export function isCanvasConnectionValid(definition: FlowDefinition, connection: Connection | Edge): boolean {
  const sourceIndex = definition.steps.findIndex((step) => step.id === connection.source);
  if (sourceIndex < 0 || connection.targetHandle !== "in") return false;
  const step = definition.steps[sourceIndex];
  const handles = step.kind === "condition" ? ["true", "false"] : ["next"];
  if (!handles.includes(connection.sourceHandle ?? "")) return false;
  return connection.target === END || definition.steps.findIndex((target) => target.id === connection.target) > sourceIndex;
}

export function graphToDefinition(base: FlowDefinition, graph: CanvasGraph): FlowDefinition {
  const stepNodes = graph.nodes.filter((node) => !node.data.virtual);
  const ids = new Set(stepNodes.map((node) => node.id));
  if (ids.size !== stepNodes.length || stepNodes.some((node) => node.id !== node.data.step?.id || !base.steps.some((step) => step.id === node.id))) {
    throw new Error("FLOW_CANVAS_INVALID_NODE");
  }
  const steps = base.steps.filter((step) => ids.has(step.id));
  const definition = { ...base, steps };
  const ports = new Set<string>();
  for (const edge of graph.edges) {
    if (edge.source === START) {
      if (edge.sourceHandle !== "next" || edge.targetHandle !== "in" || edge.target !== (steps[0]?.id ?? END)) throw new Error("FLOW_CANVAS_INVALID_START");
    } else if (!isCanvasConnectionValid(definition, edge)) throw new Error("FLOW_CANVAS_INVALID_CONNECTION");
    const port = `${edge.source}:${edge.sourceHandle}`;
    if (ports.has(port)) throw new Error("FLOW_CANVAS_DUPLICATE_PORT");
    ports.add(port);
  }
  const target = (id: string, handle: string) => graph.edges.find((edge) => edge.source === id && edge.sourceHandle === handle)?.target ?? END;
  return { ...definition, steps: steps.map((step, index) => {
    if (step.kind === "condition") return { ...step, ifTrue: target(step.id, "true"), ifFalse: target(step.id, "false"), ...(step.next && step.next !== END && !ids.has(step.next) ? { next: END } : {}) };
    const next = target(step.id, "next");
    // Preserve implicit fallthrough exactly on round trip; explicit links remain explicit.
    return step.next == null && next === (steps[index + 1]?.id ?? END) ? step : { ...step, next };
  }) };
}

export function removeCanvasStep(definition: FlowDefinition, id: string): FlowDefinition {
  if (removalImpact(definition, id).references.length) throw new Error("FLOW_CANVAS_REFERENCED_STEP");
  const graph = definitionToGraph(definition);
  graph.nodes = graph.nodes.filter((node) => node.id !== id || Boolean(node.data.virtual));
  graph.edges = graph.edges.filter((edge) => edge.source !== id).map((edge) => ({
    ...edge, target: edge.source === START ? graph.nodes.find((node) => node.data.step)?.id ?? END : edge.target === id ? END : edge.target,
  }));
  return graphToDefinition(definition, graph);
}

// Preserve other ports before changing array order; positions never affect execution.
export function insertCanvasStep(definition: FlowDefinition, edgeId: string, step: FlowStep): FlowDefinition {
  const graph = definitionToGraph(definition);
  const edge = graph.edges.find((item) => item.id === edgeId);
  if (!edge || definition.steps.length >= 100 || definition.steps.some((item) => item.id === step.id)) throw new Error("FLOW_CANVAS_INVALID_INSERT");
  const index = edge.source === START ? 0 : definition.steps.findIndex((item) => item.id === edge.source) + 1;
  const inserted = step.kind === "condition" ? { ...step, ifTrue: edge.target, ifFalse: edge.target } : { ...step, next: edge.target };
  const steps = [...definition.steps];
  steps.splice(index, 0, inserted);
  const base = { ...definition, steps };
  const nextGraph = definitionToGraph(base);
  nextGraph.edges = [
    ...graph.edges.map((item) => item.id === edgeId ? { ...item, target: step.id } : item),
    ...nextGraph.edges.filter((item) => item.source === step.id),
  ];
  return graphToDefinition(base, nextGraph);
}

export function removalImpact(definition: FlowDefinition, id: string) {
  const prefix = `/steps/${id.replace(/~/g, "~0").replace(/\//g, "~1")}`;
  const matches = (pointer: string) => pointer === prefix || pointer.startsWith(prefix + "/");
  const references = (value: unknown): boolean => {
    if (typeof value === "string") return [...value.matchAll(/\$\{([^}]+)\}/g)].some((match) => matches(match[1]));
    if (!value || typeof value !== "object") return false;
    if ("$ref" in value && typeof value.$ref === "string" && matches(value.$ref)) return true;
    return Object.values(value).some(references);
  };
  return {
    incoming: definitionToGraph(definition).edges.filter((edge) => edge.target === id).map((edge) => edge.source === START ? START : definition.steps.find((step) => step.id === edge.source)!.name),
    references: definition.steps.filter((step) => step.id !== id && references(step)).map((step) => step.name),
  };
}
