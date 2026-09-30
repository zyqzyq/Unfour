import { useCallback, useEffect, useMemo, useRef, type ReactNode } from "react";
import type { FlowDefinition } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { FlowCanvas } from "./FlowCanvas";
import { FlowSidebar } from "./FlowSidebar";
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

type FlowPageProps = { workspaceId: string; onSidebarContentChange: (node: ReactNode) => void };
export function FlowPage(props: FlowPageProps) {
  return <WorkspaceFlowPage key={props.workspaceId} {...props} />;
}
function WorkspaceFlowPage({ workspaceId, onSidebarContentChange }: FlowPageProps) {
  const { t } = useI18n();
  const data = useFlowWorkspaceData(workspaceId);
  const editor = useFlowEditor(workspaceId, data.flows);
  const execution = useFlowRuns(workspaceId, editor);
  const runTrigger = useRef<HTMLButtonElement>(null);
  const backToEditor = useRef<HTMLButtonElement>(null);
  const { draft, busy, dirty, resetSelection: resetEditor, setPendingSelection } = editor;
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
  const { data: definitions, isPending, isError, refetch } = data.flows;
  const sidebar = useMemo(() => (
    <FlowSidebar workspaceId={workspaceId} definitions={definitions} pending={isPending} failed={isError} retry={refetch} selectedFlowId={draft?.id} select={select} />
  ), [workspaceId, definitions, isPending, isError, refetch, draft?.id, select]);
  useEffect(() => {
    onSidebarContentChange(sidebar);
    return () => onSidebarContentChange(null);
  }, [sidebar, onSidebarContentChange]);

  if (!draft) return (
    <div className="flex h-full items-center justify-center text-sm text-[var(--u-color-text-muted)]">
      {t("flow.selectFlow")}
    </div>
  );
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
