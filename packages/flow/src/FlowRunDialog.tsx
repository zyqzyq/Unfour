import type { RefObject } from "react";
import type { FlowDefinition } from "@unfour/command-client";
import { Button, Dialog, DialogContent, DialogHeader, DialogTitle, DialogBody, DialogFooter, DialogDescription, Input, Select, useI18n } from "@unfour/ui";
import { RunInputs } from "./InputEditor";
import { JsonField } from "./JsonField";
import { maskInputs } from "./model";
import type { FlowEditor } from "./useFlowEditor";
import type { FlowRuns } from "./useFlowRuns";
import type { FlowWorkspaceData } from "./useFlowWorkspaceData";
import type { FlowPageValidation } from "./flowPageValidation";

export function FlowRunDialog({ workspaceId, draft, editor, execution, data, validation, runTrigger, backToEditor }: {
  workspaceId: string; draft: FlowDefinition; editor: FlowEditor; execution: FlowRuns;
  data: FlowWorkspaceData; validation: FlowPageValidation;
  runTrigger: RefObject<HTMLButtonElement | null>; backToEditor: RefObject<HTMLButtonElement | null>;
}) {
  const { t } = useI18n();
  const { runDialog, busy, setRunDialog, environmentId, setEnvironmentId, contextRevision, setInputs, secretInputNames, setSecretInputNames, error } = editor;
  const { viewingRun } = execution;
  const { resourcesQuery, environments, workspaceVariables } = data;
  const { resolvedInputs, inputProblems, manualSecretErrors, resourceProblems, environmentInputProblems, needsWorkspaceDefaults, hasApi, apiProblems, manualSecrets, invalidRun } = validation;
  return (
    <Dialog open={runDialog} onOpenChange={(open) => { if (!busy) setRunDialog(open); }}>
      <DialogContent aria-describedby={undefined} onCloseAutoFocus={(event) => { event.preventDefault(); (viewingRun ? backToEditor.current : runTrigger.current)?.focus(); }}>
        <DialogHeader><DialogTitle>{t("flow.run")}</DialogTitle></DialogHeader>
        <DialogBody><fieldset disabled={busy} className="space-y-3">
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
        <RunInputs key={`fields:${contextRevision}`} definitions={draft.inputs} values={resolvedInputs} onChange={setInputs} onValidity={editor.runValidity} />
        {inputProblems.map((problem, index) => <p key={index} role="alert" className="text-xs text-[var(--u-color-danger)]">{problem.name}: {t(problem.key)}</p>)}
        <details><summary>{t("flow.jsonInputsHelp")}</summary><JsonField
          key={`json:${contextRevision}`}
          label={t("flow.runInputs")}
          value={resolvedInputs}
          onValidity={(valid) => editor.runValidity("json", valid)}
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
        {resourceProblems.map((problem, index) => <p key={index} role="alert">{problem.name}: {t(problem.key)}</p>)}
        {environmentInputProblems.map((problem) => <p key={`${problem.name}:${problem.key}`} role="alert">{problem.name}: {t(problem.key)}</p>)}
        {(resourcesQuery.isPending || environments.isPending || ((needsWorkspaceDefaults || hasApi) && workspaceVariables.isPending)) && <p>{t("flow.loading")}</p>}
        {(resourcesQuery.isError || environments.isError) && <div role="alert">{t("flow.loadFailed")} <Button size="sm" variant="secondary" onClick={() => { void resourcesQuery.refetch(); void environments.refetch(); void workspaceVariables.refetch(); }}>{t("flow.retry")}</Button></div>}
        {environmentId && environments.isSuccess && !environments.data.some((environment) => environment.id === environmentId) && <p role="alert">{t("flow.environmentMissing")}</p>}
        {(needsWorkspaceDefaults || hasApi) && workspaceVariables.isError && <div role="alert">{t("flow.variablesLoadFailed")} <Button size="sm" variant="secondary" onClick={() => void workspaceVariables.refetch()}>{t("flow.retry")}</Button></div>}
        <p className="text-xs text-[var(--u-color-text-muted)]">{t("flow.runAgainHelp")}</p>
        <DialogDescription>{t("flow.effectsHelp")}</DialogDescription>
        <p>{t("flow.name")}: {draft.name} · {t("flow.workspace")}: {workspaceId}</p>
        <p>{t("flow.environment")}: {environments.data?.find((environment) => environment.id === environmentId)?.name ?? t("flow.workspaceOnly")}{environmentId ? ` (${environmentId})` : ""}</p>
        {apiProblems.map((problem) => <p role="alert" key={`${problem.stepId}:${problem.variable}`}>{t("flow.apiVariableUnavailable", { step: problem.name, variable: problem.variable, environment: environments.data?.find((env) => env.id === environmentId)?.name ?? t("flow.workspaceOnly") })}</p>)}
        <pre aria-label={t("flow.inputPreview")} className="max-h-40 overflow-auto whitespace-pre-wrap break-all text-xs">{JSON.stringify(maskInputs(resolvedInputs, draft.inputs, manualSecrets), null, 2)}</pre>
        {error && <p role="alert">{error}</p>}
        </fieldset></DialogBody>
        <DialogFooter>
          <Button variant="ghost" disabled={busy} onClick={() => setRunDialog(false)}>{t("common.confirm.cancel")}</Button>
          <Button disabled={busy || invalidRun || environments.isPending} onClick={() => void execution.startRun(validation)}>{t("flow.run")}</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
