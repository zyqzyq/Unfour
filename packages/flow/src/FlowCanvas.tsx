import { useState } from "react";
import { applyNodeChanges, Background, Handle, MarkerType, Position, ReactFlow, type Connection, type Edge, type NodeProps, type ReactFlowInstance } from "@xyflow/react";
import { Button, Select, useI18n } from "@unfour/ui";
import type { FlowDefinition, FlowRun } from "@unfour/command-client";
import { definitionToGraph, graphToDefinition, isCanvasConnectionValid, insertCanvasStep, type CanvasLayout, type CanvasNode } from "./canvasGraph";
import { newStep } from "./model";
import "@xyflow/react/dist/style.css";
import "./flowCanvas.css";

function StepNode({ data, selected }: NodeProps<CanvasNode>) {
  const { t } = useI18n();
  const step = data.step;
  const kind = step?.kind === "action" ? step.action.capability : step?.kind;
  return <div data-status={data.status} className={`flow-canvas-node${selected ? " is-selected" : ""}`}>
    {data.virtual !== "start" && <Handle id="in" type="target" position={Position.Left} />}
    <div className="text-xs text-[var(--u-color-text-muted)]">{step ? `${data.order + 1} · ${t(`flow.${kind}`)}` : t(`flow.canvas.${data.virtual}`)}</div>
    {data.status && <div className="text-xs" role="status">{t(`flow.status.${data.status}`)}</div>}
    {step && <div className="truncate" title={step.name}>{step.name}</div>}
    {data.virtual !== "end" && (step?.kind === "condition" ? <>
      <span className="flow-canvas-port flow-canvas-true">{t("flow.canvas.true")}</span>
      <Handle id="true" type="source" position={Position.Right} style={{ top: "35%" }} />
      <span className="flow-canvas-port flow-canvas-false">{t("flow.canvas.false")}</span>
      <Handle id="false" type="source" position={Position.Right} style={{ top: "75%" }} />
    </> : <Handle id="next" type="source" position={Position.Right} isConnectable={data.virtual !== "start"} />)}
  </div>;
}
const nodeTypes = { flowStep: StepNode };

export function FlowCanvas({ definition, onChange, disabled = false, selected, onSelect, onRemove, run }: {
  selected: string | null; onSelect: (id: string | null) => void; onRemove: (id: string) => void; run?: FlowRun;
  definition: FlowDefinition; onChange: (definition: FlowDefinition) => void; disabled?: boolean;
}) {
  const { t } = useI18n();
  const [presentation, setPresentation] = useState<CanvasNode[]>([]);
  const [layout, setLayout] = useState<CanvasLayout>({});
  const setSelected = onSelect;
  const [selectedEdge, setSelectedEdge] = useState<string | null>(null);
  const [invalid, setInvalid] = useState(false);
  const [instance, setInstance] = useState<ReactFlowInstance<CanvasNode, Edge> | null>(null);
  const graph = definitionToGraph(definition, layout);
  const insertionEdge = graph.edges.find((edge) => edge.id === selectedEdge) ?? graph.edges[graph.edges.length - 1];
  const nodes = graph.nodes.map((node) => {
    const previous = presentation.find((item) => item.id === node.id);
    return { ...node, data: { ...node.data, status: run?.steps.find((step) => step.stepId === node.id)?.status }, measured: previous?.measured, selected: node.id === selected, ariaLabel: node.data.step?.name ?? t(`flow.canvas.${node.data.virtual}`) };
  });
  const selectedStep = definition.steps.find((step) => step.id === selected);
  function editGraph(action: () => FlowDefinition) {
    try {
      const next = action();
      setInvalid(false);
      onChange(next);
      return true;
    } catch {
      setInvalid(true);
      return false;
    }
  }
  function connect(connection: Connection) {
    if (disabled || !isCanvasConnectionValid(definition, connection)) return;
    const edges = graph.edges.filter((edge) => edge.source !== connection.source || edge.sourceHandle !== connection.sourceHandle);
    editGraph(() => graphToDefinition(definition, { ...graph, edges: [...edges, { ...connection, id: `${connection.source}:${connection.sourceHandle}` }] }));
  }
  return <section className="flow-canvas flex min-h-0 min-w-0 flex-1 flex-col border-b border-[var(--u-color-border)]" aria-label={t("flow.canvas.title")}>
    <div className="flex flex-wrap items-center gap-2 p-2">
      <Select className="!w-40" aria-label={t("flow.addStep")} value="" disabled={disabled || definition.steps.length >= 100} options={[
        { value: "", label: t("flow.canvas.add") },
        ...["api", "database", "ssh", "condition", "waitUntil", "wait"].map((value) => ({ value, label: t(`flow.${value}`) })),
      ]} onChange={(event) => {
        if (event.target.value) {
          const step = newStep(event.target.value, t(`flow.${event.target.value}`));
          if (editGraph(() => insertCanvasStep(definition, insertionEdge.id, step))) {
            setSelected(step.id); setSelectedEdge(null);
          }
        }
      }} />
      <Button variant="ghost" size="sm" disabled={disabled || !selectedStep || definition.steps.length <= 1} onClick={() => {
        if (selectedStep) onRemove(selectedStep.id);
      }}>{t("flow.canvas.remove")}</Button>
      <Button variant="ghost" size="sm" disabled={disabled || !graph.edges.some((edge) => edge.id === selectedEdge && edge.deletable)} onClick={() => {
        editGraph(() => graphToDefinition(definition, { ...graph, edges: graph.edges.filter((edge) => edge.id !== selectedEdge) }));
        setSelectedEdge(null);
      }}>{t("flow.canvas.disconnect")}</Button>
      <Button variant="ghost" size="sm" onClick={() => void instance?.zoomIn()}>{t("flow.canvas.zoomIn")}</Button>
      <Button variant="ghost" size="sm" onClick={() => void instance?.zoomOut()}>{t("flow.canvas.zoomOut")}</Button>
      <Button variant="ghost" size="sm" onClick={() => void instance?.fitView()}>{t("flow.canvas.fit")}</Button>
    </div>
    <label className="flex items-center gap-2 px-2 pb-2 text-xs"><span className="shrink-0">{t("flow.canvas.insertOn")}</span><Select aria-label={t("flow.canvas.insertOn")} value={insertionEdge.id} options={graph.edges.map((edge) => ({ value: edge.id, label: `${nodes.find((node) => node.id === edge.source)?.ariaLabel} ${edge.sourceHandle === "true" || edge.sourceHandle === "false" ? t(`flow.canvas.${edge.sourceHandle}`) : ""} → ${nodes.find((node) => node.id === edge.target)?.ariaLabel ?? edge.target}` }))} onChange={(event) => setSelectedEdge(event.target.value)} /></label>
    <p className="px-2 pb-2 text-xs text-[var(--u-color-text-muted)]">{t("flow.canvas.help")}</p>
    {invalid && <p role="alert" className="px-2 text-xs text-[var(--u-color-danger)]">{t("flow.canvas.invalid")}</p>}
    <div className="min-h-64 flex-1">
      <ReactFlow<CanvasNode, Edge> nodes={nodes}
        edges={graph.edges.map((edge) => ({ ...edge, selected: edge.id === selectedEdge, markerEnd: { type: MarkerType.ArrowClosed }, ariaLabel: `${nodes.find((node) => node.id === edge.source)?.ariaLabel} → ${nodes.find((node) => node.id === edge.target)?.ariaLabel}`, label: edge.sourceHandle === "true" || edge.sourceHandle === "false" ? t(`flow.canvas.${edge.sourceHandle}`) : undefined }))}
        nodeTypes={nodeTypes} onInit={setInstance} fitView minZoom={0.15} maxZoom={2}
        nodesConnectable={!disabled} nodesDraggable={!disabled} edgesReconnectable={false} deleteKeyCode={null}
        onNodesChange={(changes) => {
          setPresentation(applyNodeChanges(changes, nodes));
          const selection = changes.find((change) => change.type === "select" && change.selected);
          if (selection?.type === "select") { setSelected(selection.id); setSelectedEdge(null); }
          if (changes.some((change) => change.type === "position" && change.position)) setLayout((previous) => {
            const next = { ...previous };
            for (const change of changes) if (change.type === "position" && change.position) next[change.id] = change.position;
            return next;
          });
        }}
        onNodeClick={(_, node) => { setSelected(node.id); setSelectedEdge(null); }}
        onEdgeClick={(_, edge) => { setSelectedEdge(edge.id); setSelected(null); }}
        onPaneClick={() => { setSelected(null); setSelectedEdge(null); }}
        isValidConnection={(connection) => !disabled && isCanvasConnectionValid(definition, connection)} onConnect={connect}>
        <Background />
      </ReactFlow>
    </div>
  </section>;
}
