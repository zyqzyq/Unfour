import type { FlowDefinition } from "@unfour/command-client";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowRuns } from "./useFlowRuns";
import type { FlowPageValidation } from "./flowPageValidation";
import type { FlowWorkspaceData } from "./useFlowWorkspaceData";
import { Button, useI18n } from "@unfour/ui";
import { START } from "./canvasGraph";

export function FlowProblems({ draft, editor, execution, validation, resourcesQuery }: {
  draft: FlowDefinition; editor: FlowEditor; execution: FlowRuns; validation: FlowPageValidation;
  resourcesQuery: FlowWorkspaceData["resourcesQuery"];
}) {
  const { t } = useI18n();
  const { busy, error, revisionConflict, invalid, setSelectedNode } = editor;
  const { viewingRun } = execution;
  const { referenceProblems, boundsProblems, resourceProblems, invalidEditor, schemaProblems } = validation;
  const runBlockedReason = execution.blockedReason(validation);
  return (
    <>
      {!viewingRun && runBlockedReason && <p className="px-2 py-1 text-xs text-[var(--u-color-text-muted)]">{runBlockedReason}</p>}
      <p className="px-2 py-1 text-xs text-[var(--u-color-text-muted)]">{t("flow.environmentScope")}</p>
      <div className="max-h-40 shrink-0 overflow-auto break-words">
      {!viewingRun && referenceProblems.map((problem) => <p key={`${problem.stepId}:${problem.key}:${problem.pointer}`} role="alert" className="px-2 text-xs text-[var(--u-color-danger)]"><Button size="sm" variant="ghost" onClick={() => setSelectedNode(problem.stepId)}>{problem.name}</Button>{t(problem.key)} · {problem.pointer.split("/").slice(1).map((part, index) => index === 1 ? draft.steps.find((step) => step.id === part)?.name ?? part : part).join(" · ")}</p>)}
      {error && <p role="alert" className="break-words p-2 text-[var(--u-color-danger)]">{error}</p>}
      {revisionConflict && <div role="alert" className="px-2 py-1">
        <p>{t("flow.revisionConflict")}</p>
        <Button size="sm" variant="secondary" disabled={busy} onClick={() => void editor.reloadLatest()}>{t("flow.reloadLatest")}</Button>
      </div>}
      {resourcesQuery.isError && <div role="alert" className="p-2">{t("flow.loadFailed")} <Button size="sm" variant="secondary" onClick={() => void resourcesQuery.refetch()}>{t("flow.retry")}</Button></div>}
      {!viewingRun && [...boundsProblems, ...resourceProblems].map((problem, index) => <p key={index} role="alert" className="px-2 py-1 text-xs text-[var(--u-color-danger)]">
        {problem.stepId && <Button size="sm" variant="ghost" className="max-w-full whitespace-normal break-all text-left" onClick={() => setSelectedNode(problem.stepId!)}>{draft.steps.find((step) => step.id === problem.stepId)?.name}</Button>}{t(problem.key)}
      </p>)}
      {invalidEditor && !viewingRun && <div role="alert" className="px-2 py-1 text-xs text-[var(--u-color-danger)]">
        {t("flow.invalidEditors")}
        {(schemaProblems.length > 0 || Object.entries(invalid).some(([key, value]) => value && key.startsWith("schema:"))) && <Button size="sm" variant="ghost" onClick={() => setSelectedNode(START)}>{t("flow.canvas.start")}</Button>}
        {draft.steps.filter((step) => !referenceProblems.some((problem) => problem.stepId === step.id) && !resourceProblems.some((problem) => problem.stepId === step.id) && !boundsProblems.some((problem) => problem.stepId === step.id) && Object.entries(invalid).some(([key, value]) => value && key.startsWith(step.id + ":"))).map((step) => <Button key={step.id} size="sm" variant="ghost" onClick={() => setSelectedNode(step.id)}>{step.name}</Button>)}
      </div>}
      </div>
    </>
  );
}
