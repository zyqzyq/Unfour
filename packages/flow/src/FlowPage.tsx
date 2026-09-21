import { START, END, removalImpact, removeCanvasStep } from "./canvasGraph";
import { inputDefaults, inputDefinitionErrors, inputErrors, maskInputs, newStep, normalizeInputs, resourceErrors } from "./model";
import { InputEditor, RunInputs } from "./InputEditor";
import { RunView } from "./RunView";
import { FlowCanvas } from "./FlowCanvas";
import {
  useCallback,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  cancelFlowRun,
  deleteFlow,
  listFlows,
  listFlowRuns,
  getFlowRun,
  listDatabaseConnections,
  listSavedApiRequests,
  listSshConnections,
  listSshTasks,
  listWorkspaceEnvironments,
  runFlow,
  saveFlow,
  type FlowDefinition,
  type FlowRun,
} from "@unfour/command-client";
import {
  Button,
  ConfirmDialog,
  Input,
  Select,
  SidebarHeader,
  SidebarRow,
  useI18n,
} from "@unfour/ui";
import { JsonField, StepEditor, type Resources } from "./StepEditor";

type FlowPageProps = { workspaceId: string; onSidebarContentChange: (node: ReactNode) => void };
export function FlowPage(props: FlowPageProps) {
  return <WorkspaceFlowPage key={props.workspaceId} {...props} />;
}
function WorkspaceFlowPage({
  workspaceId,
  onSidebarContentChange,
}: FlowPageProps) {
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
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [confirm, setConfirm] = useState<"run" | "delete" | null>(null);
  const [selectedRun, setSelectedRun] = useState<string | null>(null);
  const [pendingSelection, setPendingSelection] =
    useState<FlowDefinition | null>(null);
  const flows = useQuery({
    queryKey: ["flows", workspaceId],
    queryFn: () => listFlows(workspaceId),
  });
  const resourcesQuery = useQuery({
    queryKey: ["flow-resources", workspaceId],
    queryFn: async (): Promise<Resources> => {
      const [api, ssh, database, connections] = await Promise.all([
        listSavedApiRequests(workspaceId),
        listSshTasks(workspaceId),
        listDatabaseConnections(workspaceId),
        listSshConnections(workspaceId),
      ]);
      return { api, ssh, database, connections };
    },
  });
  const environments = useQuery({
    queryKey: ["workspace-environments", workspaceId],
    queryFn: () => listWorkspaceEnvironments(workspaceId),
  });
  const runs = useQuery({
    queryKey: ["flow-runs", workspaceId, draft?.id],
    queryFn: () => listFlowRuns(workspaceId, draft!.id),
    enabled: Boolean(draft?.id),
  });
  const runDetail = useQuery({
    queryKey: ["flow-run", workspaceId, selectedRun],
    queryFn: () => getFlowRun(workspaceId, selectedRun!),
    enabled: Boolean(selectedRun),
    refetchInterval: (query) =>
      query.state.data?.status === "running" ? 500 : false,
  });
  const currentRun =
    runDetail.data ?? runs.data?.find((run) => run.id === selectedRun);
  useEffect(() => {
    if (!runDetail.data) return;
    const updated = runDetail.data;
    queryClient.setQueryData<FlowRun[]>(
      ["flow-runs", workspaceId, updated.flowId],
      (history) =>
        history?.map((run) => (run.id === updated.id ? updated : run)),
    );
  }, [runDetail.data, queryClient, workspaceId]);
  const resources = resourcesQuery.data ?? {
    api: [],
    ssh: [],
    database: [],
    connections: [],
  };
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
    setSelectedRun(null);
    setConfirm(null);
    setError("");
    setContextRevision((revision) => revision + 1);
  }, []);
  const select = useCallback(
    (flow: FlowDefinition) => {
      if (busy) return;
      if (dirty) {
        setPendingSelection(flow);
        return;
      }
      resetSelection(flow);
    },
    [busy, dirty, resetSelection],
  );
  const sidebar = useMemo(
    () => (
      <>
        <SidebarHeader>{t("flow.title")}</SidebarHeader>
        <Button
          className="m-2"
          variant="secondary"
          size="sm"
          onClick={() =>
            select({
              id: "",
              workspaceId,
              name: t("flow.new"),
              revision: 0,
              inputs: [],
              steps: [newStep("api", t("flow.api"))],
            })
          }
        >
          {t("flow.new")}
        </Button>
        {flows.isPending && <p className="p-2 text-xs">{t("flow.loading")}</p>}
        {flows.isError && (
          <p role="alert" className="p-2">
            {t("flow.loadFailed")}
          </p>
        )}
        {flows.data?.map((flow) => (
          <SidebarRow
            key={flow.id}
            active={draft?.id === flow.id}
            onClick={() => select(flow)}
          >
            {flow.name}
          </SidebarRow>
        ))}
      </>
    ),
    [
      t,
      workspaceId,
      flows.data,
      flows.isPending,
      flows.isError,
      draft?.id,
      select,
    ],
  );
  useEffect(() => {
    onSidebarContentChange(sidebar);
    return () => onSidebarContentChange(null);
  }, [sidebar, onSidebarContentChange]);
  function update(value: FlowDefinition) {
    setDraft(value);
    setDirty(true);
  }
  async function perform(action: () => Promise<void>) {
    setBusy(true);
    setError("");
    try {
      await action();
    } catch (cause) {
      const detail = cause as { code?: string; message?: string };
      setError(
        `${t("flow.operationFailed")} ${detail.code ?? ""} ${detail.message ?? String(cause)}`,
      );
    } finally {
      setBusy(false);
    }
  }
  if (!draft)
    return (
      <div className="flex h-full items-center justify-center text-sm text-[var(--u-color-text-muted)]">
        {t("flow.selectFlow")}
      </div>
    );
  const schemaProblems = inputDefinitionErrors(draft.inputs);
  const invalidEditor = schemaProblems.length > 0 || Object.entries(invalid).some(([key, value]) => !key.startsWith("run:") && value);
  const resolvedInputs = { ...inputDefaults(draft.inputs), ...inputs };
  const inputProblems = inputErrors(draft.inputs, resolvedInputs);
  const resourceProblems = resourcesQuery.data ? resourceErrors(draft, resources) : [];
  const manualSecrets = secretInputNames.split(",").map((name) => name.trim()).filter(Boolean);
  const manualSecretErrors = manualSecrets.filter((name) => !Object.prototype.hasOwnProperty.call(resolvedInputs, name));
  const impact = removingNode ? removalImpact(draft, removingNode) : null;
  const invalidRun = manualSecretErrors.length > 0 || invalidEditor || inputProblems.length > 0 || resourceProblems.length > 0 || Object.values(invalid).some(Boolean) || !resourcesQuery.data || resourcesQuery.isError || environments.isError || (Boolean(environmentId) && !environments.data?.some((environment) => environment.id === environmentId));
  return (
    <div className="flex h-full min-h-0 flex-col text-[13px]">
      <div className="flex shrink-0 items-center gap-2 border-b border-[var(--u-color-border)] p-2">
        <Input
          aria-label={t("flow.name")}
          disabled={busy}
          value={draft.name}
          onChange={(e) => update({ ...draft, name: e.target.value })}
        />
        <span className="shrink-0 whitespace-nowrap">
          {dirty ? t("flow.unsaved") : `r${draft.revision}`}
        </span>
        <Button
          variant="secondary"
          disabled={busy || invalidEditor}
          onClick={() =>
            void perform(async () => {
              const saved = await saveFlow(draft);
              setDraft(saved);
              setDirty(false);
              await queryClient.invalidateQueries({
                queryKey: ["flows", workspaceId],
              });
            })
          }
        >
          {t("flow.save")}
        </Button>
        <Button
          disabled={busy || dirty || !draft.id || invalidRun}
          onClick={() => setConfirm("run")}
        >
          {t("flow.run")}
        </Button>
        <Button
          variant="ghost"
          disabled={busy || !draft.id}
          onClick={() => setConfirm("delete")}
        >
          {t("flow.delete")}
        </Button>
      </div>
      {(error ||
        resourcesQuery.isError ||
        runs.isError ||
        runDetail.isError) && (
        <p role="alert" className="p-2 text-[var(--u-color-danger)]">
          {error || t("flow.loadFailed")}
        </p>
      )}
      {invalidEditor && <div role="alert" className="px-2 py-1 text-xs text-[var(--u-color-danger)]">
        {t("flow.invalidEditors")}
        {(schemaProblems.length > 0 || Object.entries(invalid).some(([key, value]) => value && key.startsWith("schema:"))) && <Button size="sm" variant="ghost" onClick={() => setSelectedNode(START)}>{t("flow.canvas.start")}</Button>}
        {draft.steps.filter((step) => Object.entries(invalid).some(([key, value]) => value && key.startsWith(step.id + ":"))).map((step) => <Button key={step.id} size="sm" variant="ghost" onClick={() => setSelectedNode(step.id)}>{step.name}</Button>)}
      </div>}
      <div className="flex min-h-0 flex-1 overflow-hidden">
        <FlowCanvas key={contextRevision} definition={draft} disabled={busy} selected={selectedNode} onSelect={setSelectedNode} onRemove={setRemovingNode} run={!dirty && currentRun?.definition.revision === draft.revision ? currentRun : undefined} onChange={update} />
        <aside aria-label={t("flow.inspector")} hidden={!selectedNode} className="w-[360px] shrink-0 overflow-auto border-l border-[var(--u-color-border)] p-3">
          <fieldset disabled={busy}>
          <div className="flex items-center justify-between"><strong>{t("flow.inspector")}</strong><Button variant="ghost" size="sm" onClick={() => setSelectedNode(null)}>{t("flow.closeInspector")}</Button></div>
          <div hidden={selectedNode !== START}>
          <InputEditor key={contextRevision} definitions={draft.inputs} onChange={(definitions) => {
            const validRunKeys = new Set(definitions.filter((field) => draft.inputs.some((previous) => previous.name === field.name && previous.type === field.type && previous.secret === field.secret)).map((field) => "run:" + field.name));
            setInvalid((state) => Object.fromEntries(Object.entries(state).filter(([key]) => !key.startsWith("run:") || key === "run:json" || validRunKeys.has(key))));
            update({ ...draft, inputs: definitions });
          }} onValidity={(key, valid) => {
            if (!valid) setDirty(true);
            setInvalid((state) => ({ ...state, [`schema:${key}`]: !valid }));
          }} />
          </div>
          {schemaProblems.map((problem, index) => <p key={index} role="alert" className="py-1 text-xs text-[var(--u-color-danger)]">{problem.name}: {t(problem.key)}</p>)}
          {resourceProblems.map((problem, index) => <p key={index} role="alert" className="py-1 text-xs text-[var(--u-color-danger)]">{problem.name}: {t(problem.key)}</p>)}
          {draft.steps.map((step, index) => (
            <div key={`${contextRevision}:${step.id}`} hidden={selectedNode !== step.id}><StepEditor
              removeDisabled={draft.steps.length <= 1}
              inputs={draft.inputs}
              before={draft.steps.slice(0, index)}
              key={`${contextRevision}:${step.id}`}
              step={step}
              after={draft.steps.slice(index + 1)}
              resources={resources}
              onValidity={(field, valid) => {
                if (!valid) setDirty(true);
                setInvalid((state) => ({
                  ...Object.fromEntries(Object.entries(state).filter(([key]) => !valid || !key.startsWith(`${step.id}:${field}:`))),
                  [`${step.id}:${field}`]: !valid,
                }));
              }}
              onRemove={() => setRemovingNode(step.id)}
              onChange={(value) =>
                update({
                  ...draft,
                  steps: draft.steps.map((s) => (s.id === step.id ? value : s)),
                })
              }
            /></div>
          ))}
          {selectedNode === END && <p>{t("flow.endHelp")}</p>}
          </fieldset>
        </aside>
      </div>
      <details open className="max-h-[35%] shrink-0 overflow-auto border-t border-[var(--u-color-border)]">
        <summary className="cursor-pointer p-2">{t("flow.runPanel")}</summary>
        <div className="space-y-3 overflow-auto p-3">
          <label>
            {t("flow.environment")}
            <Select
              value={environmentId}
              onChange={(e) => setEnvironmentId(e.target.value)}
              options={[
                { value: "", label: t("flow.workspaceOnly") },
                ...(environments.data ?? []).map((e) => ({
                  value: e.id,
                  label: e.name,
                })),
              ]}
            />
          </label>
          <RunInputs key={`fields:${contextRevision}`} definitions={draft.inputs} values={resolvedInputs} onChange={setInputs} onValidity={(key, valid) => setInvalid((state) => ({ ...state, [`run:${key}`]: !valid }))} />
          {inputProblems.map((problem, index) => <p key={index} role="alert" className="text-xs text-[var(--u-color-danger)]">{problem.name}: {t(problem.key)}</p>)}
          <details><summary>{t("flow.jsonInputsHelp")}</summary><JsonField
            key={`json:${contextRevision}`}
            label={t("flow.runInputs")}
            value={resolvedInputs}
            onValidity={(valid) =>
              setInvalid((s) => ({ ...s, "run:json": !valid }))
            }
            onChange={(value) => {
              if (value && typeof value === "object" && !Array.isArray(value))
                setInputs(value as Record<string, unknown>);
              else return false;
            }}
          /></details>
          <details><summary>{t("flow.advanced")}</summary><label>
            {t("flow.secretInputs")}
            <Input
              aria-label={t("flow.secretInputs")}
              value={secretInputNames}
              onChange={(e) => setSecretInputNames(e.target.value)}
            />
          </label>
          </details>
          {manualSecretErrors.length > 0 && <p role="alert">{t("flow.unknownSecretInput")}: {manualSecretErrors.join(", ")}</p>}
          <label>
            {t("flow.history")}
            <Select
              aria-label={t("flow.history")}
              value={selectedRun ?? ""}
              onChange={(e) => setSelectedRun(e.target.value || null)}
              options={[
                { value: "", label: t("flow.selectRun") },
                ...(runs.data ?? []).map((run) => ({
                  value: run.id,
                  label: `${run.startedAt} · ${t(`flow.status.${run.status}`)}`,
                })),
              ]}
            />
          </label>
          {currentRun && (
            <RunView
              run={currentRun}
              cancel={() =>
                void perform(async () => {
                  await cancelFlowRun(workspaceId, currentRun.id);
                  await runs.refetch();
                })
              }
            />
          )}
        </div>
      </details>
      <ConfirmDialog
        open={confirm !== null}
        onOpenChange={(open) => {
          if (!open) setConfirm(null);
        }}
        title={t(confirm === "delete" ? "flow.delete" : "flow.run")}
        description={confirm === "delete" ? t("flow.deleteHelp") : <span className="grid gap-2">
          <span>{t("flow.effectsHelp")}</span>
          <span>{t("flow.name")}: {draft.name}</span>
          <span>{t("flow.workspace")}: {workspaceId}</span>
          <span>{t("flow.environment")}: {environments.data?.find((environment) => environment.id === environmentId)?.name ?? t("flow.workspaceOnly")}{environmentId ? ` (${environmentId})` : ""}</span>
          <span>{t("flow.inputValues")}</span>
          <code className="max-h-60 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify(maskInputs(resolvedInputs, draft.inputs, manualSecrets), null, 2)}</code>
        </span>}
        confirmLabel={t(confirm === "delete" ? "flow.delete" : "flow.run")}
        pending={busy}
        onConfirm={() =>
          void perform(async () => {
            if (confirm === "delete") {
              await deleteFlow(workspaceId, draft.id);
              setDraft(null);
              setDirty(false);
              await flows.refetch();
            } else {
              if (invalidRun) return;
              const run = await runFlow({
                workspaceId,
                flowId: draft.id,
                environmentId: environmentId || null,
                inputs: resolvedInputs,
                secretInputNames: manualSecrets,
                initiator: "human",
                confirmEffects: true,
              });
              setSelectedRun(run.id);
              await runs.refetch();
            }
            setConfirm(null);
          })
        }
      />
      <ConfirmDialog open={removingNode !== null} onOpenChange={(open) => { if (!open) setRemovingNode(null); }} title={t("flow.canvas.remove")} description={<span className="grid gap-2"><span>{t("flow.canvas.removeHelp")}</span><span>{t("flow.canvas.incoming")}: {impact?.incoming.map((name) => name === START ? t("flow.canvas.start") : name).join(", ") || t("flow.none")}</span>{Boolean(impact?.references.length) && <span role="alert">{t("flow.canvas.referenced")}: {impact?.references.join(", ")}</span>}</span>} confirmLabel={t(impact?.references.length ? "flow.acknowledge" : "flow.remove")} onConfirm={() => {
        if (impact?.references.length) { setRemovingNode(null); return; }
        if (!removingNode || draft.steps.length <= 1) return;
        update(removeCanvasStep(draft, removingNode));
        setInvalid((state) => Object.fromEntries(Object.entries(state).filter(([key]) => !key.startsWith(removingNode + ":"))));
        setSelectedNode(START); setRemovingNode(null);
      }} />
      <ConfirmDialog
        open={pendingSelection !== null}
        onOpenChange={(open) => {
          if (!open) setPendingSelection(null);
        }}
        title={t("flow.unsaved")}
        description={t("flow.discardHelp")}
        confirmLabel={t("flow.discard")}
        onConfirm={() => {
          if (pendingSelection) resetSelection(pendingSelection);
          setPendingSelection(null);
        }}
      />
    </div>
  );
}
