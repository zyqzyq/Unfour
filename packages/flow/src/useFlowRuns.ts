import { useCallback, useEffect, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { cancelFlowRun, getFlowRun, listFlowRuns, runFlow } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { START } from "./canvasGraph";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowPageValidation } from "./flowPageValidation";

export function useFlowRuns(workspaceId: string, editor: FlowEditor) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [historyOpen, setHistoryOpen] = useState(false);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const { draft, dirty, busy } = editor;
  const runs = useQuery({
    queryKey: ["flow-runs", workspaceId, draft?.id],
    queryFn: () => listFlowRuns(workspaceId, draft!.id),
    enabled: Boolean(draft?.id) && historyOpen,
    refetchInterval: (query) => query.state.data?.some((run) => run.status === "running") ? 500 : false,
  });
  const runDetail = useQuery({
    queryKey: ["flow-run", workspaceId, selectedRun],
    queryFn: () => getFlowRun(workspaceId, selectedRun!),
    enabled: Boolean(selectedRun),
    refetchInterval: (query) => query.state.data?.status === "running" ? 500 : false,
  });
  const currentRun = runDetail.data;
  useEffect(() => {
    if (runDetail.data && runDetail.data.status !== "running") {
      void queryClient.invalidateQueries({ queryKey: ["flow-runs", workspaceId, runDetail.data.flowId] });
    }
  }, [runDetail.data, queryClient, workspaceId]);

  const resetSelection = useCallback(() => {
    setHistoryOpen(false);
    setSelectedRun(null);
  }, []);
  function selectRun(id: string) {
    setSelectedRun(id);
    editor.setSelectedNode(START);
  }
  function backToEditor() {
    setSelectedRun(null);
    editor.restoreEditorSelection();
  }
  function editStep(id: string) {
    setSelectedRun(null);
    editor.setSelectedNode(id);
  }
  function runAgain() {
    setSelectedRun(null);
    editor.setSelectedNode(START);
    editor.openRun();
  }
  function startRun(validation: FlowPageValidation) {
    if (!draft) return;
    return editor.perform(async () => {
      if (validation.invalidRun || dirty) return;
      const run = await runFlow({
        workspaceId, flowId: draft.id, environmentId: editor.environmentId || null,
        inputs: validation.resolvedInputs, secretInputNames: validation.manualSecrets,
        initiator: "human", confirmEffects: true,
      }, draft.revision);
      queryClient.setQueryData(["flow-run", workspaceId, run.id], run);
      selectRun(run.id);
      editor.setRunDialog(false);
      await queryClient.invalidateQueries({ queryKey: ["flow-runs", workspaceId, draft.id] });
    });
  }
  function cancelRun() {
    if (!currentRun || !draft) return;
    return editor.perform(async () => {
      await cancelFlowRun(workspaceId, currentRun.id);
      await runDetail.refetch();
      await queryClient.invalidateQueries({ queryKey: ["flow-runs", workspaceId, draft.id] });
    });
  }
  const viewingRun = selectedRun !== null;
  const attentionStep = currentRun?.steps.find((step) => ["running", "failed", "timedOut", "interrupted"].includes(step.status));
  const projection = !dirty && currentRun?.flowId === draft?.id && currentRun?.definition.revision === draft?.revision ? currentRun : undefined;
  function blockedReason(validation: FlowPageValidation) {
    return busy ? t("flow.operationPending") : viewingRun ? t("flow.backToEditor")
      : dirty || !draft?.id ? t("flow.saveBeforeRun")
        : validation.invalidEditor || validation.resourceProblems.length > 0 ? t("flow.invalidEditors") : "";
  }
  return {
    runs, runDetail, currentRun, selectedRun, setHistoryOpen, resetSelection,
    selectRun, backToEditor, editStep, runAgain, startRun, cancelRun,
    viewingRun, attentionStep, projection, blockedReason,
  };
}

export type FlowRuns = ReturnType<typeof useFlowRuns>;
