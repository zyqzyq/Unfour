import { useContext } from "react";
import { BaseEdge, EdgeLabelRenderer, getBezierPath, type EdgeProps } from "@xyflow/react";
import { Plus } from "lucide-react";
import { Button, DropdownMenu, DropdownMenuTrigger, DropdownMenuContent, DropdownMenuItem, useI18n } from "@unfour/ui";

import { CanvasInsertion } from "./canvasInsertion";

export function CanvasInsertEdge(props: EdgeProps) {
  const { t } = useI18n();
  const { disabled, insert } = useContext(CanvasInsertion);
  const [path, x, y] = getBezierPath(props);
  const branch = props.sourceHandleId === "true" || props.sourceHandleId === "false" ? t(`flow.canvas.${props.sourceHandleId}`) : "";
  // Branches can converge on the same target. Keep their controls near the
  // distinct source ports so the two insertion menus never cover each other.
  const controlX = branch ? props.sourceX + 28 : x;
  const controlY = branch ? props.sourceY + (props.sourceHandleId === "true" ? -20 : 20) : y;
  return <>
    <BaseEdge id={props.id} path={path} markerEnd={props.markerEnd} style={props.style} />
    <EdgeLabelRenderer><div onClick={(event) => event.stopPropagation()} onPointerDown={(event) => event.stopPropagation()} className="nodrag nopan flex items-center gap-1" style={{ position: "absolute", transform: `translate(-50%, -50%) translate(${controlX}px, ${controlY}px)`, pointerEvents: "all" }}>
      {branch && <span className="pointer-events-none absolute bottom-full left-1/2 -translate-x-1/2 bg-[var(--u-color-surface)] text-xs">{branch}</span>}
      <DropdownMenu><DropdownMenuTrigger asChild><Button variant="secondary" size="sm" className="!h-6 !w-6 !p-0" disabled={disabled} aria-label={`${t("flow.canvas.add")} · ${props.data?.description ?? props.id}`} title={t("flow.canvas.add")}><Plus size={12} /></Button></DropdownMenuTrigger>
        <DropdownMenuContent>{["api", "database", "ssh", "condition", "waitUntil", "wait"].map((kind) => <DropdownMenuItem key={kind} onSelect={() => insert(props.id, kind)}>{t(`flow.${kind}`)}</DropdownMenuItem>)}</DropdownMenuContent>
      </DropdownMenu>
    </div></EdgeLabelRenderer>
  </>;
}
