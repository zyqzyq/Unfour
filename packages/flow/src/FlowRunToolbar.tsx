import type { RefObject } from "react";
import { Button, useI18n } from "@unfour/ui";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowRuns } from "./useFlowRuns";
import type { FlowPageValidation } from "./flowPageValidation";
import { flowDisplayStatus } from "./runStatus";

export function FlowRunToolbar({ editor, execution, validation, backToEditor }: {
  editor: FlowEditor; execution: FlowRuns; validation: FlowPageValidation;
  backToEditor: RefObject<HTMLButtonElement | null>;
}) {
  const { t } = useI18n();
  const { busy, dirty, setSelectedNode } = editor;
  const { viewingRun, currentRun, runDetail, attentionStep, projection } = execution;
  const { invalidEditor, resourceProblems } = validation;
  if (!viewingRun) return null;
  return (
    <div className="flex shrink-0 flex-wrap items-center gap-2 border-b border-[var(--u-color-border)] bg-[var(--u-color-primary-soft)] px-4 py-2.5">
      <strong className="text-xs text-[var(--u-color-primary)]">{t("flow.viewingRun")}</strong>
      <span className="text-xs text-[var(--u-color-text-muted)]">{currentRun ? `${currentRun.startedAt} · ${t(`flow.status.${flowDisplayStatus(currentRun)}`)} · r${currentRun.definition.revision}` : t(runDetail.isError ? "flow.loadFailed" : "flow.loading")}</span>
      {runDetail.isError && <div role="alert">{t("flow.loadFailed")} <Button size="sm" variant="secondary" onClick={() => void runDetail.refetch()}>{t("flow.retry")}</Button></div>}
      <Button ref={backToEditor} variant="secondary" size="sm" onClick={execution.backToEditor}>{t("flow.backToEditor")}</Button>
      {attentionStep && <Button className="max-w-full whitespace-normal break-all text-left" variant="secondary" size="sm" onClick={() => setSelectedNode(attentionStep.stepId)}>{t("flow.locateStep", { name: currentRun?.definition.steps.find((step) => step.id === attentionStep.stepId)?.name ?? attentionStep.stepId })} · {t(`flow.status.${flowDisplayStatus(attentionStep)}`)}</Button>}
      {currentRun && currentRun.status !== "running" && <Button variant="secondary" size="sm" disabled={busy || dirty || invalidEditor || resourceProblems.length > 0} title={dirty ? t("flow.saveBeforeRun") : t("flow.runAgainHelp")} onClick={execution.runAgain}>{t("flow.runAgain")}</Button>}
      {currentRun?.status === "running" && <Button variant="secondary" size="sm" disabled={busy} onClick={() => void execution.cancelRun()}>{t("flow.cancel")}</Button>}
      {currentRun && !projection && <p className="w-full text-xs text-[var(--u-color-text-muted)]">{t("flow.runProjectionUnavailable")}</p>}
      {currentRun && currentRun.status !== "running" && (dirty || invalidEditor || resourceProblems.length > 0) && <p className="w-full text-xs">{t("flow.runAgainBlocked")}</p>}
    </div>
  );
}
