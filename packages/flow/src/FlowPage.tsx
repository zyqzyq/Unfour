import { useCallback, useEffect, useMemo, useRef, type ReactNode } from "react";
import type { FlowDefinition, RedisFlowIntent } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { FlowCanvas } from "./FlowCanvas";
import { FlowSidebar } from "./FlowSidebar";
import { FlowStart } from "./FlowStart";
import { newStep } from "./model";
import { FlowToolbar } from "./FlowToolbar";
import { FlowProblems } from "./FlowProblems";
import { FlowRunToolbar } from "./FlowRunToolbar";
import { FlowInspector } from "./FlowInspector";
import { FlowRunDialog } from "./FlowRunDialog";
import { FlowEditorDialogs } from "./FlowEditorDialogs";
import { useFlowWorkspaceData } from "./useFlowWorkspaceData";
import { useFlowEditor } from "./useFlowEditor";
import { useFlowRuns } from "./useFlowRuns";
import { getFlowPageValidation } from "./flowPageValidation";

type FlowPageProps = {
  workspaceId: string; onSidebarContentChange: (node: ReactNode) => void; onDirtyChange?: (dirty: boolean) => void;
  redisIntent?: RedisFlowIntent | null; onRedisIntentHandled?: () => void;
};
export function FlowPage(props: FlowPageProps) {
  return <WorkspaceFlowPage key={props.workspaceId} {...props} />;
}
function WorkspaceFlowPage({ workspaceId, onSidebarContentChange, onDirtyChange, redisIntent, onRedisIntentHandled }: FlowPageProps) {
  const { t } = useI18n();
  const data = useFlowWorkspaceData(workspaceId);
  const editor = useFlowEditor(workspaceId, data.flows, data.environments);
  const execution = useFlowRuns(workspaceId, editor);
  const runTrigger = useRef<HTMLButtonElement>(null);
  const backToEditor = useRef<HTMLButtonElement>(null);
  const { draft, busy, dirty, resetSelection: resetEditor, setPendingSelection } = editor;
  // Report the editor's dirty bit, including raw inputs rejected by parsing.
  // Hidden modules remain mounted, so their leave protection remains active.
  useEffect(() => { onDirtyChange?.(dirty); }, [dirty, onDirtyChange]);
  useEffect(() => () => onDirtyChange?.(false), [onDirtyChange]);
  const { resetSelection: resetRuns } = execution;
  // Reset both contexts in the same interaction, before any new Flow is rendered.
  const resetSelection = useCallback((flow: FlowDefinition) => {
    resetEditor(flow);
    resetRuns();
  }, [resetEditor, resetRuns]);
  const select = useCallback((flow: FlowDefinition) => {
    if (busy) return;
    if (dirty) {
      setPendingSelection({ flow });
      return;
    }
    resetSelection(flow);
  }, [busy, dirty, resetSelection, setPendingSelection]);
  const handled = useRef<RedisFlowIntent | null>(null);
  const { setInputs } = editor;
  useEffect(() => {
    if (!redisIntent || redisIntent === handled.current || busy || redisIntent.workspaceId !== workspaceId) return;
    handled.current = redisIntent;
    const step = newStep("redis", t("flow.redis"));
    if (step.kind !== "action") return;
    const flow: FlowDefinition = {
      id: "", workspaceId, name: t("flow.redis"), revision: 0,
      inputs: [{ name: "cacheKey", type: "string", required: true, secret: true }],
      steps: [{ ...step, action: { ...step.action, resourceId: redisIntent.connectionId, arguments: { key: { $ref: "/inputs/cacheKey" } } } }],
    };
    const runtimeInputs = { cacheKey: redisIntent.key };
    if (dirty) setPendingSelection({ flow, runtimeInputs });
    else { resetSelection(flow); setInputs(runtimeInputs); }
    onRedisIntentHandled?.();
  }, [busy, dirty, redisIntent, onRedisIntentHandled, resetSelection, setInputs, setPendingSelection, t, workspaceId]);
  const { data: definitions, isPending, isError, refetch } = data.flows;
  const sidebar = useMemo(() => (
    <FlowSidebar workspaceId={workspaceId} definitions={definitions} pending={isPending} failed={isError} busy={busy} retry={refetch} selectedFlowId={draft?.id} select={select} />
  ), [workspaceId, definitions, isPending, isError, busy, refetch, draft?.id, select]);
  useEffect(() => {
    onSidebarContentChange(sidebar);
    return () => onSidebarContentChange(null);
  }, [sidebar, onSidebarContentChange]);

  if (!draft) return <FlowStart
    pending={isPending}
    failed={isError}
    hasFlows={Boolean(definitions?.length)}
    refreshing={data.flows.isFetching}
    onRetry={() => void refetch()}
    onCreate={() => select({ id: "", workspaceId, name: t("flow.new"), revision: 0, inputs: [], steps: [newStep("api", t("flow.api"))] })}
  />;
  const validation = getFlowPageValidation(draft, editor, data);
  return (
    <div className="flow-page flex h-full min-h-0 min-w-0 flex-col text-[13px]">
      <FlowToolbar draft={draft} editor={editor} execution={execution} validation={validation} runTrigger={runTrigger} />
      <FlowProblems draft={draft} editor={editor} execution={execution} validation={validation} resourcesQuery={data.resourcesQuery} />
      <FlowRunToolbar editor={editor} execution={execution} validation={validation} backToEditor={backToEditor} />
      <div className="flow-workbench flex min-h-0 flex-1 overflow-hidden">
        <FlowCanvas resources={data.resources} key={editor.contextRevision} definition={draft} disabled={busy} readOnly={execution.viewingRun} selected={editor.selectedNode} onSelect={editor.setSelectedNode} onRemove={editor.setRemovingNode} run={execution.viewingRun ? execution.projection : undefined} onChange={editor.update} />
        <FlowInspector draft={draft} editor={editor} execution={execution} validation={validation} resources={data.resources} />
      </div>
      <FlowRunDialog workspaceId={workspaceId} draft={draft} editor={editor} execution={execution} data={data} validation={validation} runTrigger={runTrigger} backToEditor={backToEditor} />
      <FlowEditorDialogs editor={editor} resetSelection={resetSelection} />
    </div>
  );
}
