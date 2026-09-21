import type { FlowDefinition, FlowStep } from "@unfour/command-client";
import type { Connection, Edge, Node, XYPosition } from "@xyflow/react";

export const START = "$start";
export const END = "$end";
export type CanvasNode = Node<{ step?: FlowStep; virtual?: "start" | "end"; order: number }, "flowStep">;
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
    if (step.kind === "condition") return { ...step, ifTrue: target(step.id, "true"), ifFalse: target(step.id, "false"), next: step.next && !ids.has(step.next) ? END : step.next };
    const next = target(step.id, "next");
    // Preserve implicit fallthrough exactly on round trip; explicit links remain explicit.
    return { ...step, next: step.next == null && next === (steps[index + 1]?.id ?? END) ? null : next };
  }) };
}

export function removeCanvasStep(definition: FlowDefinition, id: string): FlowDefinition {
  const graph = definitionToGraph(definition);
  graph.nodes = graph.nodes.filter((node) => node.id !== id || Boolean(node.data.virtual));
  graph.edges = graph.edges.filter((edge) => edge.source !== id).map((edge) => ({
    ...edge, target: edge.source === START ? graph.nodes.find((node) => node.data.step)?.id ?? END : edge.target === id ? END : edge.target,
  }));
  return graphToDefinition(definition, graph);
}
