import { useCallback, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { deleteFlow, getFlow, saveFlow, type FlowDefinition, type FlowInputDefinition, type FlowStep } from "@unfour/command-client";
import { useI18n } from "@unfour/ui";
import { START, END, removalImpact, removeCanvasStep } from "./canvasGraph";
import { inputDefaults, normalizeInputs } from "./model";
import type { FlowWorkspaceData } from "./useFlowWorkspaceData";

export function useFlowEditor(workspaceId: string, flows: FlowWorkspaceData["flows"]) {
  const { t } = useI18n();
  const queryClient = useQueryClient();
  const [draft, setDraft] = useState<FlowDefinition | null>(null);
  const [dirty, setDirty] = useState(false);
  const [selectedNode, setSelectedNode] = useState<string | null>(START);
  const [removingNode, setRemovingNode] = useState<string | null>(null);
  const [contextRevision, setContextRevision] = useState(0);
  const [inputs, setInputs] = useState<Record<string, unknown>>({});
  const [invalid, setInvalid] = useState<Record<string, boolean>>({});
  const [secretInputNames, setSecretInputNames] = useState("");
  const [environmentId, setEnvironmentId] = useState("");
  const [runDialog, setRunDialog] = useState(false);
  const [revisionConflict, setRevisionConflict] = useState(false);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<"delete" | null>(null);
  const [pendingSelection, setPendingSelection] = useState<{ flow: FlowDefinition; latest?: boolean } | null>(null);

  // Selection and explicit discard share one reset. Query refreshes never replace a draft.
  const resetSelection = useCallback((flow: FlowDefinition) => {
    const normalized = { ...flow, inputs: normalizeInputs(flow.inputs) };
    setDraft(normalized);
    setSelectedNode(START);
    setRemovingNode(null);
    setDirty(!flow.id);
    setInputs(inputDefaults(normalized.inputs));
    setSecretInputNames("");
    setEnvironmentId("");
    setInvalid({});
    setRunDialog(false);
    setConfirm(null);
    setError("");
    setRevisionConflict(false);
    setContextRevision((revision) => revision + 1);
  }, []);

  function update(value: FlowDefinition) {
    setDraft(value);
    setDirty(true);
  }
  // Save, delete, run and cancel use the same pending/error gate as before.
  async function perform(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (cause) {
      const detail = cause as { code?: string; message?: string };
      if (detail.code === "FLOW_CONFIRMATION_STALE") {
        setRevisionConflict(true);
        setRunDialog(false);
        setError(t("flow.confirmationStale"));
        return;
      }
      if (detail.code === "FLOW_REVISION_CONFLICT") setRevisionConflict(true);
      setError(`${t("flow.operationFailed")} ${detail.code ?? ""} ${detail.message ?? String(cause)}`);
    } finally {
      setBusy(false);
    }
  }
  function save() {
    if (!draft) return;
    return perform(async () => {
      const saved = await saveFlow(draft);
      setDraft(saved);
      setDirty(false);
      setRevisionConflict(false);
      await queryClient.invalidateQueries({ queryKey: ["flows", workspaceId] });
    });
  }
  function deleteDefinition() {
    if (!draft) return;
    return perform(async () => {
      await deleteFlow(workspaceId, draft.id);
      setDraft(null);
      setDirty(false);
      setConfirm(null);
      await flows.refetch();
    });
  }
  function reloadLatest() {
    if (!draft) return;
    return perform(async () => {
      const latest = await getFlow(workspaceId, draft.id);
      setPendingSelection({ flow: latest, latest: true });
    });
  }
  function discardSelection(reset: (flow: FlowDefinition) => void) {
    if (pendingSelection) {
      if (pendingSelection.latest) {
        queryClient.setQueryData<FlowDefinition[]>(["flows", workspaceId], (cached) =>
          cached?.map((flow) => flow.id === pendingSelection.flow.id ? pendingSelection.flow : flow));
      }
      reset(pendingSelection.flow);
    }
    setPendingSelection(null);
  }
  function changeInputDefinitions(definitions: FlowInputDefinition[]) {
    if (!draft) return;
    const validRunKeys = new Set(definitions.filter((field) => draft.inputs.some((previous) =>
      previous.name === field.name && previous.type === field.type && previous.secret === field.secret)).map((field) => "run:" + field.name));
    setInvalid((state) => Object.fromEntries(Object.entries(state).filter(([key]) =>
      !key.startsWith("run:") || key === "run:json" || validRunKeys.has(key))));
    update({ ...draft, inputs: definitions });
  }
  function schemaValidity(key: string, valid: boolean) {
    if (!valid) setDirty(true);
    setInvalid((state) => ({ ...state, [`schema:${key}`]: !valid }));
  }
  function stepValidity(stepId: string, field: string, valid: boolean) {
    if (!valid) setDirty(true);
    setInvalid((state) => ({
      ...Object.fromEntries(Object.entries(state).filter(([key]) => !valid || !key.startsWith(`${stepId}:${field}:`))),
      [`${stepId}:${field}`]: !valid,
    }));
  }
  function changeStep(value: FlowStep) {
    if (draft) update({ ...draft, steps: draft.steps.map((step) => step.id === value.id ? value : step) });
  }
  const impact = draft && removingNode ? removalImpact(draft, removingNode) : null;
  function removeStep() {
    if (impact?.references.length) { setRemovingNode(null); return; }
    if (!draft || !removingNode || draft.steps.length <= 1) return;
    update(removeCanvasStep(draft, removingNode));
    setInvalid((state) => Object.fromEntries(Object.entries(state).filter(([key]) => !key.startsWith(removingNode + ":"))));
    setSelectedNode(START);
    setRemovingNode(null);
  }
  function openRun() {
    setInvalid((state) => Object.fromEntries(Object.entries(state).filter(([key]) => !key.startsWith("run:"))));
    setRunDialog(true);
  }
  function runValidity(key: string, valid: boolean) {
    setInvalid((state) => ({ ...state, [`run:${key}`]: !valid }));
  }
  function restoreEditorSelection() {
    if (draft && selectedNode !== END && !draft.steps.some((step) => step.id === selectedNode)) setSelectedNode(START);
  }
  return {
    draft, dirty, selectedNode, setSelectedNode, removingNode, setRemovingNode, contextRevision,
    inputs, setInputs, invalid, secretInputNames, setSecretInputNames, environmentId, setEnvironmentId,
    runDialog, setRunDialog, revisionConflict, error, busy, confirm, setConfirm, pendingSelection, setPendingSelection,
    resetSelection, update, perform, save, deleteDefinition, reloadLatest, discardSelection,
    changeInputDefinitions, schemaValidity, stepValidity, changeStep, impact, removeStep, openRun, runValidity, restoreEditorSelection,
  };
}

export type FlowEditor = ReturnType<typeof useFlowEditor>;
