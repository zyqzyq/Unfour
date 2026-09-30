import type { FlowDefinition } from "@unfour/command-client";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowRuns } from "./useFlowRuns";
import type { FlowPageValidation } from "./flowPageValidation";
import type { Resources } from "./model";
import { Button, useI18n } from "@unfour/ui";
import { START, END } from "./canvasGraph";
import { InputEditor } from "./InputEditor";
import { StepEditor } from "./StepEditor";
import { RunView } from "./RunView";

export function FlowInspector({ draft, editor, execution, validation, resources }: {
  draft: FlowDefinition; editor: FlowEditor; execution: FlowRuns; validation: FlowPageValidation; resources: Resources;
}) {
  const { t } = useI18n();
  const { busy, selectedNode, setSelectedNode, setRemovingNode, contextRevision } = editor;
  const { viewingRun, currentRun, runDetail } = execution;
  const { schemaProblems, upstream, environmentKeys } = validation;
  return (
    <aside aria-label={t(viewingRun ? "flow.runDetail" : "flow.inspector")} hidden={!selectedNode} className="flow-inspector w-[360px] min-w-0 shrink-0 overflow-auto border-l border-[var(--u-color-border)] p-3">
      {viewingRun && <>
        <div className="flex items-center justify-between"><strong>{t("flow.runDetail")}</strong><Button variant="ghost" size="sm" onClick={() => setSelectedNode(null)}>{t("flow.closeInspector")}</Button></div>
        {currentRun ? <RunView key={`${currentRun.id}:${selectedNode}`} run={currentRun} selectedStep={selectedNode} onSelectStep={setSelectedNode} onEditStep={execution.editStep} editableStepIds={draft.steps.map((step) => step.id)} /> : !runDetail.isError && <p>{t("flow.loading")}</p>}
      </>}
      <fieldset disabled={busy || viewingRun} hidden={viewingRun}>
      <div className="flex items-center justify-between"><strong>{t("flow.inspector")}</strong><Button variant="ghost" size="sm" onClick={() => setSelectedNode(null)}>{t("flow.closeInspector")}</Button></div>
      <div hidden={selectedNode !== START}>
      <InputEditor key={contextRevision} definitions={draft.inputs} onChange={editor.changeInputDefinitions} onValidity={editor.schemaValidity} />
      </div>
      {schemaProblems.map((problem, index) => <p key={index} role="alert" className="py-1 text-xs text-[var(--u-color-danger)]">{problem.name}: {t(problem.key)}</p>)}
      {draft.steps.map((step, index) => (
        <div key={`${contextRevision}:${step.id}`} hidden={selectedNode !== step.id}><StepEditor
          removeDisabled={draft.steps.length <= 1}
          inputs={draft.inputs}
          before={draft.steps.filter((candidate) => upstream.get(step.id)?.has(candidate.id))}
          environmentKeys={environmentKeys}
          key={`${contextRevision}:${step.id}`}
          step={step}
          after={draft.steps.slice(index + 1)}
          resources={resources}
          onValidity={(field, valid) => editor.stepValidity(step.id, field, valid)}
          onRemove={() => setRemovingNode(step.id)}
          onChange={editor.changeStep}
        /></div>
      ))}
      {selectedNode === END && <p>{t("flow.endHelp")}</p>}
      </fieldset>
    </aside>
  );
}
