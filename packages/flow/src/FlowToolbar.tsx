import type { FlowDefinition } from "@unfour/command-client";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowRuns } from "./useFlowRuns";
import type { FlowPageValidation } from "./flowPageValidation";
import type { RefObject } from "react";
import { Button, Input, useI18n } from "@unfour/ui";
import { Play, Save, Trash2, Workflow } from "lucide-react";
import { RunHistory } from "./RunHistory";

export function FlowToolbar({ draft, editor, execution, validation, runTrigger }: {
  draft: FlowDefinition; editor: FlowEditor; execution: FlowRuns; validation: FlowPageValidation;
  runTrigger: RefObject<HTMLButtonElement | null>;
}) {
  const { t } = useI18n();
  const { busy, dirty, contextRevision, update, setConfirm } = editor;
  const { viewingRun, runs, selectedRun, setHistoryOpen } = execution;
  const { invalidEditor, resourceProblems } = validation;
  const runBlockedReason = execution.blockedReason(validation);
  return (
    <div className="flow-toolbar flex shrink-0 flex-wrap items-center gap-2 border-b border-[var(--u-color-border)] px-4 py-2">
      <span className="grid h-8 w-8 shrink-0 place-items-center rounded-[var(--u-radius-md)] bg-[var(--u-color-primary-soft)] text-[var(--u-color-primary)]"><Workflow aria-hidden="true" size={17} /></span>
      <Input
        className="flow-title-input min-w-32 flex-1"
        aria-label={t("flow.name")}
        disabled={busy || viewingRun}
        value={draft.name}
        onChange={(e) => update({ ...draft, name: e.target.value })}
      />
      <span className="shrink-0 whitespace-nowrap rounded-[var(--u-radius-sm)] border border-[var(--u-color-border)] px-2 py-1 font-mono text-[10px] text-[var(--u-color-text-muted)]">
        {dirty ? t("flow.unsaved") : `r${draft.revision}`}
      </span>
      <Button
        variant="secondary"
        disabled={busy || viewingRun || invalidEditor}
        onClick={() => void editor.save()}
      >
        <Save aria-hidden="true" size={14} />{t("flow.save")}
      </Button>
      <Button
        ref={runTrigger}
        title={runBlockedReason || undefined}
        disabled={busy || viewingRun || dirty || !draft.id || invalidEditor || resourceProblems.length > 0}
        onClick={editor.openRun}
      >
        <Play aria-hidden="true" size={14} />{t("flow.run")}
      </Button>
      <RunHistory key={contextRevision} runs={runs.data ?? []} loading={runs.isPending} failed={runs.isError} disabled={busy || !draft.id} onRetry={() => void runs.refetch()} selected={selectedRun} onSelect={execution.selectRun} onOpenChange={setHistoryOpen} />
      <Button
        variant="ghost"
        disabled={busy || viewingRun || !draft.id}
        onClick={() => setConfirm("delete")}
      >
        <Trash2 aria-hidden="true" size={14} />{t("flow.delete")}
      </Button>
    </div>
  );
}
